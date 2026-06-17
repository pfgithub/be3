use std::{
    collections::BTreeMap,
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use logicgame::{
    challenges::ChallengeId,
    execution::{Component, GenerationError, UnlinkedComponent, Vm},
    grid::{
        ComponentHash, ComponentKind, ComponentPort, ComponentSide, GeometryError, LogicGrid,
        LogicGridSnapshot, Point, Size,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum ComponentFileError {
    InvalidName(&'static str),
    AlreadyExists(String),
    Io(io::Error),
    Json(serde_json::Error),
    Generation(GenerationError),
    InvalidSubcomponent(&'static str),
    Geometry(GeometryError),
}

impl fmt::Display for ComponentFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(message) => formatter.write_str(message),
            Self::AlreadyExists(name) => write!(formatter, "A component named \"{name}\" exists"),
            Self::Io(error) => write!(formatter, "File error: {error}"),
            Self::Json(error) => write!(formatter, "Invalid component file: {error}"),
            Self::Generation(error) => write!(formatter, "Cannot compile component: {error:?}"),
            Self::InvalidSubcomponent(message) => formatter.write_str(message),
            Self::Geometry(error) => write!(formatter, "Invalid component shape: {error:?}"),
        }
    }
}

impl From<io::Error> for ComponentFileError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for ComponentFileError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<GenerationError> for ComponentFileError {
    fn from(error: GenerationError) -> Self {
        Self::Generation(error)
    }
}

impl From<GeometryError> for ComponentFileError {
    fn from(error: GeometryError) -> Self {
        Self::Geometry(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompiledComponent {
    pub snapshot: LogicGridSnapshot,
    pub component: UnlinkedComponent,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ComponentFileRef {
    Component(String),
    ChallengeSolution {
        challenge: ChallengeId,
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentFileDrag {
    pub file: ComponentFileRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeSolutionFile {
    pub name: String,
    pub passing: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChallengeProgress {
    pub passed: BTreeMap<ChallengeId, bool>,
}

impl ChallengeProgress {
    pub fn is_passed(&self, id: ChallengeId) -> bool {
        self.passed.get(&id).copied().unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct StoredFile {
    kind: StoredFileKind,
    grid: LogicGridSnapshot,
    passing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum StoredFileKind {
    Component,
    ChallengeSolution { challenge: ChallengeId },
}

#[derive(Clone)]
pub struct ComponentFiles {
    root: PathBuf,
    compiled_root: PathBuf,
}

impl ComponentFiles {
    pub fn new(root: PathBuf) -> Self {
        let compiled_root = root.parent().map_or_else(
            || PathBuf::from("compiled"),
            |parent| parent.join("compiled"),
        );
        Self {
            root,
            compiled_root,
        }
    }

    pub fn list(&self) -> Result<Vec<String>, ComponentFileError> {
        match fs::read_dir(&self.root) {
            Ok(entries) => {
                let mut names = Vec::new();
                for entry in entries {
                    let entry = entry?;
                    let path = entry.path();
                    if !entry.file_type()?.is_file()
                        || path.extension().and_then(|value| value.to_str()) != Some("json")
                    {
                        continue;
                    }
                    if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
                        if validate_name(name).is_ok()
                            && self
                                .load_stored(&ComponentFileRef::Component(name.to_owned()))
                                .is_ok_and(|stored| {
                                    matches!(stored.kind, StoredFileKind::Component)
                                })
                        {
                            names.push(name.to_owned());
                        }
                    }
                }
                names.sort_by_key(|name| name.to_lowercase());
                Ok(names)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn list_challenge_solutions(
        &self,
        challenge: ChallengeId,
    ) -> Result<Vec<ChallengeSolutionFile>, ComponentFileError> {
        let root = self.challenge_root(challenge);
        match fs::read_dir(&root) {
            Ok(entries) => {
                let mut solutions = Vec::new();
                for entry in entries {
                    let entry = entry?;
                    let path = entry.path();
                    if !entry.file_type()?.is_file()
                        || path.extension().and_then(|value| value.to_str()) != Some("json")
                    {
                        continue;
                    }
                    let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    if validate_name(name).is_err() {
                        continue;
                    }
                    let stored = self.load_stored(&ComponentFileRef::ChallengeSolution {
                        challenge,
                        name: name.to_owned(),
                    })?;
                    if stored.kind != (StoredFileKind::ChallengeSolution { challenge }) {
                        continue;
                    }
                    solutions.push(ChallengeSolutionFile {
                        name: name.to_owned(),
                        passing: stored.passing,
                    });
                }
                solutions.sort_by_key(|solution| solution.name.to_lowercase());
                Ok(solutions)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    pub fn create(&self, name: &str) -> Result<LogicGrid, ComponentFileError> {
        let name = validate_name(name)?;
        fs::create_dir_all(&self.root)?;
        let path = self.path(name);
        if path.exists() {
            return Err(ComponentFileError::AlreadyExists(name.to_owned()));
        }
        let grid = LogicGrid::new();
        self.save(name, &grid)?;
        Ok(grid)
    }

    pub fn create_challenge_solution(
        &self,
        challenge: ChallengeId,
    ) -> Result<(String, LogicGrid), ComponentFileError> {
        fs::create_dir_all(self.challenge_root(challenge))?;
        let existing = self.list_challenge_solutions(challenge)?;
        let mut index = 1;
        loop {
            let name = format!("Solution {index}");
            if !existing.iter().any(|solution| solution.name == name) {
                let grid = LogicGrid::new();
                self.save_challenge_solution(challenge, &name, &grid, false)?;
                return Ok((name, grid));
            }
            index += 1;
        }
    }

    pub fn load(&self, name: &str) -> Result<LogicGrid, ComponentFileError> {
        let name = validate_name(name)?;
        let stored = self.load_stored(&ComponentFileRef::Component(name.to_owned()))?;
        Ok(LogicGrid::from_snapshot(stored.grid))
    }

    pub fn load_ref(&self, file: &ComponentFileRef) -> Result<LogicGrid, ComponentFileError> {
        Ok(LogicGrid::from_snapshot(self.load_stored(file)?.grid))
    }

    pub fn save(&self, name: &str, grid: &LogicGrid) -> Result<(), ComponentFileError> {
        let name = validate_name(name)?;
        fs::create_dir_all(&self.root)?;
        self.save_stored(
            &ComponentFileRef::Component(name.to_owned()),
            &StoredFile {
                kind: StoredFileKind::Component,
                grid: grid.snapshot(),
                passing: false,
            },
        )?;
        Ok(())
    }

    pub fn save_challenge_solution(
        &self,
        challenge: ChallengeId,
        name: &str,
        grid: &LogicGrid,
        passing: bool,
    ) -> Result<(), ComponentFileError> {
        let name = validate_name(name)?;
        self.save_stored(
            &ComponentFileRef::ChallengeSolution {
                challenge,
                name: name.to_owned(),
            },
            &StoredFile {
                kind: StoredFileKind::ChallengeSolution { challenge },
                grid: grid.snapshot(),
                passing,
            },
        )
    }

    pub fn compile_subcomponent(
        &self,
        file: &ComponentFileRef,
    ) -> Result<ComponentKind, ComponentFileError> {
        let grid = self.load_ref(file)?;
        let bounds = grid
            .bounds()
            .ok_or(ComponentFileError::InvalidSubcomponent(
                "Empty component files cannot be used as subcomponents",
            ))?;
        let width = i64::try_from(bounds.width())
            .map_err(|_| ComponentFileError::InvalidSubcomponent("Component width is too large"))?;
        let height = i64::try_from(bounds.height()).map_err(|_| {
            ComponentFileError::InvalidSubcomponent("Component height is too large")
        })?;
        let size = Size::new(width, height);
        let ports = subcomponent_ports(&grid, bounds.min, bounds.max)?;
        let snapshot = grid.snapshot();
        let component = UnlinkedComponent::from_graph(&grid, &grid.generate_graph())?;
        let bytes = serde_json::to_vec_pretty(&CompiledComponent {
            snapshot,
            component,
        })?;
        let hash = ComponentHash::new(format!("{:x}", Sha256::digest(&bytes)))
            .expect("SHA-256 is a valid component hash");

        fs::create_dir_all(&self.compiled_root)?;
        let path = self.compiled_root.join(format!("{hash}.json"));
        if !path.exists() {
            atomic_write(&path, &bytes)?;
        }

        Ok(ComponentKind::subcomponent(hash, size, ports)?)
    }

    fn compiled_path(&self, hash: &ComponentHash) -> PathBuf {
        self.compiled_root.join(format!("{hash}.json"))
    }

    pub fn load_components(&self, vm: &mut Vm) -> Result<(), ComponentFileError> {
        let mut cache = BTreeMap::<ComponentHash, Rc<Component>>::new();
        vm.load_components(|hash| self.load_component(hash, &mut cache))
    }

    pub fn load_progress(&self) -> Result<ChallengeProgress, ComponentFileError> {
        match fs::read(self.progress_path()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Ok(ChallengeProgress::default())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn save_progress(&self, progress: &ChallengeProgress) -> Result<(), ComponentFileError> {
        fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec_pretty(progress)?;
        atomic_write(&self.progress_path(), &bytes)?;
        Ok(())
    }

    fn load_component(
        &self,
        hash: &ComponentHash,
        cache: &mut BTreeMap<ComponentHash, Rc<Component>>,
    ) -> Result<Rc<Component>, ComponentFileError> {
        if let Some(component) = cache.get(hash) {
            return Ok(Rc::clone(component));
        }
        let bytes = fs::read(self.compiled_path(hash))?;
        let compiled = serde_json::from_slice::<CompiledComponent>(&bytes)?;
        let component = compiled
            .component
            .link_with_hash(hash.clone(), |child_hash| {
                self.load_component(child_hash, cache)
            })?;
        cache.insert(hash.clone(), Rc::clone(&component));
        Ok(component)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.json"))
    }

    fn challenge_root(&self, challenge: ChallengeId) -> PathBuf {
        self.root.join("challenges").join(format!("{challenge:?}"))
    }

    fn challenge_path(&self, challenge: ChallengeId, name: &str) -> PathBuf {
        self.challenge_root(challenge).join(format!("{name}.json"))
    }

    fn progress_path(&self) -> PathBuf {
        self.root.join("progress.json")
    }

    fn file_path(&self, file: &ComponentFileRef) -> Result<PathBuf, ComponentFileError> {
        match file {
            ComponentFileRef::Component(name) => Ok(self.path(validate_name(name)?)),
            ComponentFileRef::ChallengeSolution { challenge, name } => {
                Ok(self.challenge_path(*challenge, validate_name(name)?))
            }
        }
    }

    fn load_stored(&self, file: &ComponentFileRef) -> Result<StoredFile, ComponentFileError> {
        let bytes = fs::read(self.file_path(file)?)?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn save_stored(
        &self,
        file: &ComponentFileRef,
        stored: &StoredFile,
    ) -> Result<(), ComponentFileError> {
        let matches_file = match (file, &stored.kind) {
            (ComponentFileRef::Component(_), StoredFileKind::Component) => true,
            (
                ComponentFileRef::ChallengeSolution { challenge, .. },
                StoredFileKind::ChallengeSolution {
                    challenge: stored_challenge,
                },
            ) => challenge == stored_challenge,
            _ => false,
        };
        if !matches_file {
            return Err(ComponentFileError::InvalidSubcomponent(
                "File reference does not match stored file kind",
            ));
        }
        let path = self.file_path(file)?;
        fs::create_dir_all(path.parent().expect("component path has a parent"))?;
        let bytes = serde_json::to_vec_pretty(stored)?;
        atomic_write(&path, &bytes)?;
        Ok(())
    }
}

fn subcomponent_ports(
    grid: &LogicGrid,
    min: Point,
    max: Point,
) -> Result<Vec<ComponentPort>, ComponentFileError> {
    let mut ports = Vec::new();
    for component in grid.components() {
        let (direction, index, scale) = match component.kind {
            ComponentKind::Input { id, scale } => {
                (logicgame::grid::ConnectionDirection::Input, id.0, scale)
            }
            ComponentKind::Output { id, scale } => {
                (logicgame::grid::ConnectionDirection::Output, id.0, scale)
            }
            _ => continue,
        };
        let lead = component
            .lead()
            .ok_or(ComponentFileError::InvalidSubcomponent(
                "Input or output component has no external lead",
            ))?;
        let on_boundary = match lead.side {
            ComponentSide::Top => lead.axis == min.y,
            ComponentSide::Right => lead.axis == max.x,
            ComponentSide::Bottom => lead.axis == max.y,
            ComponentSide::Left => lead.axis == min.x,
        };
        if !on_boundary {
            return Err(ComponentFileError::InvalidSubcomponent(
                "Every input and output must lie on the component bounds",
            ));
        }
        let offset = match lead.side {
            ComponentSide::Top | ComponentSide::Bottom => min.x,
            ComponentSide::Right | ComponentSide::Left => min.y,
        };
        ports.push(ComponentPort {
            direction,
            index,
            scale,
            side: lead.side,
            start: lead.start - offset,
            end: lead.end - offset,
        });
    }
    ports.sort_by_key(|port| (port.direction, port.index));
    Ok(ports)
}

pub fn validate_name(name: &str) -> Result<&str, ComponentFileError> {
    if name.trim() != name {
        return Err(ComponentFileError::InvalidName(
            "Names cannot start or end with spaces",
        ));
    }
    if name.is_empty() {
        return Err(ComponentFileError::InvalidName("Enter a component name"));
    }
    if name.chars().count() > 64 {
        return Err(ComponentFileError::InvalidName(
            "Names can contain at most 64 characters",
        ));
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, ' ' | '_' | '-'))
    {
        return Err(ComponentFileError::InvalidName(
            "Use only letters, numbers, spaces, _ and -",
        ));
    }
    Ok(name)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().expect("component path has a parent");
    let temp_path = parent.join(format!(
        ".logicgame-{}-{}.tmp",
        std::process::id(),
        TEMP_FILE_ID.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        replace_file(&temp_path, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn replace_file(temp_path: &Path, path: &Path) -> io::Result<()> {
    match fs::rename(temp_path, path) {
        Ok(()) => Ok(()),
        Err(error) if path.exists() => {
            fs::remove_file(path)?;
            fs::rename(temp_path, path).map_err(|_| error)
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use logicgame::{
        execution::Instruction,
        grid::{ComponentKind, ComponentSide, Point, Rotation, Scale},
    };

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn test_root() -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "logicgame-components-{}-{}",
                std::process::id(),
                TEST_ID.fetch_add(1, Ordering::Relaxed)
            ))
            .join("components")
    }

    fn remove_test_root(root: &Path) {
        fs::remove_dir_all(root.parent().expect("test root has a parent")).unwrap();
    }

    #[test]
    fn validates_component_names() {
        for valid in ["Adder", "half adder", "mux_2-1", "A1"] {
            assert_eq!(validate_name(valid).unwrap(), valid);
        }
        for invalid in ["", " leading", "trailing ", "../escape", "emoji!"] {
            assert!(validate_name(invalid).is_err(), "{invalid:?}");
        }
        assert!(validate_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn creates_lists_saves_and_loads_components() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let mut grid = files.create("Zed").unwrap();
        files.create("alpha").unwrap();
        fs::write(root.join("notes.txt"), b"ignored").unwrap();
        assert_eq!(files.list().unwrap(), ["alpha", "Zed"]);
        assert!(matches!(
            files.create("Zed"),
            Err(ComponentFileError::AlreadyExists(_))
        ));

        grid.add_component(
            Point::new(2, 4),
            Rotation::Right,
            ComponentKind::Not {
                scale: Scale::new(2).unwrap(),
            },
        );
        files.save("Zed", &grid).unwrap();
        assert_eq!(files.load("Zed").unwrap().snapshot(), grid.snapshot());

        remove_test_root(&root);
    }

    #[test]
    fn reports_malformed_component_files() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("broken.json"), b"{").unwrap();
        let files = ComponentFiles::new(root.clone());
        assert!(matches!(
            files.load("broken"),
            Err(ComponentFileError::Json(_))
        ));
        remove_test_root(&root);
    }

    #[test]
    fn compiled_component_serializes_unlinked_component() {
        let compiled = CompiledComponent {
            snapshot: LogicGrid::new().snapshot(),
            component: UnlinkedComponent {
                memory_size: 3,
                storage_init: vec![6, 7],
                inputs: vec![2],
                outputs: vec![1],
                components: Vec::new(),
                instructions: vec![Instruction::Not {
                    input: 2,
                    output: 1,
                }],
            },
        };

        let json = serde_json::to_value(&compiled).unwrap();
        assert_eq!(json["component"]["memory_size"], 3);
        assert_eq!(json["component"]["storage_init"], serde_json::json!([6, 7]));
        assert_eq!(json["component"]["inputs"], serde_json::json!([2]));
        assert_eq!(json["component"]["outputs"], serde_json::json!([1]));
        assert!(json["component"]["instructions"].is_array());

        let decoded: CompiledComponent = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.snapshot, compiled.snapshot);
        assert_eq!(decoded.component, compiled.component);
    }

    #[test]
    fn compiles_and_reuses_content_addressed_subcomponents() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let mut grid = files.create("source").unwrap();
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::new(2).unwrap(),
                id: logicgame::grid::InputId(usize::MAX),
            },
        );
        grid.add_component(
            Point::new(4, 4),
            Rotation::Down,
            ComponentKind::Output {
                scale: Scale::new(4).unwrap(),
                id: logicgame::grid::OutputId(usize::MAX),
            },
        );
        files.save("source", &grid).unwrap();

        let source = ComponentFileRef::Component("source".to_owned());
        let first = files.compile_subcomponent(&source).unwrap();
        let second = files.compile_subcomponent(&source).unwrap();
        assert_eq!(first, second);
        let ComponentKind::Subcomponent {
            component,
            size,
            ports,
            ..
        } = first
        else {
            panic!("expected subcomponent");
        };
        assert_eq!(size, Size::new(8, 8));
        assert_eq!(
            ports,
            vec![
                ComponentPort::input(0, Scale::new(2).unwrap(), ComponentSide::Top, 0, 2),
                ComponentPort::output(0, Scale::new(4).unwrap(), ComponentSide::Bottom, 4, 8),
            ]
        );
        let path = files.compiled_path(&component);
        let bytes = fs::read(&path).unwrap();
        let compiled: CompiledComponent = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(compiled.snapshot, grid.snapshot());
        assert_eq!(compiled.component.inputs.len(), 1);
        assert_eq!(compiled.component.outputs.len(), 1);
        assert_eq!(component.as_str(), format!("{:x}", Sha256::digest(&bytes)));

        grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
        files.save("source", &grid).unwrap();
        let changed = files.compile_subcomponent(&source).unwrap();
        let ComponentKind::Subcomponent {
            component: changed_hash,
            ..
        } = changed
        else {
            panic!("expected subcomponent");
        };
        assert_ne!(changed_hash, component);
        assert!(files.compiled_path(&changed_hash).exists());

        remove_test_root(&root);
    }

    #[test]
    fn rejects_empty_and_non_boundary_subcomponents() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        files.create("empty").unwrap();
        assert!(matches!(
            files.compile_subcomponent(&ComponentFileRef::Component("empty".to_owned())),
            Err(ComponentFileError::InvalidSubcomponent(_))
        ));

        let mut grid = files.create("internal-port").unwrap();
        grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        grid.add_component(
            Point::new(4, 4),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: logicgame::grid::InputId(usize::MAX),
            },
        );
        grid.add_component(Point::new(4, -4), Rotation::Up, ComponentKind::Led);
        files.save("internal-port", &grid).unwrap();
        assert!(matches!(
            files.compile_subcomponent(&ComponentFileRef::Component("internal-port".to_owned())),
            Err(ComponentFileError::InvalidSubcomponent(_))
        ));

        remove_test_root(&root);
    }

    #[test]
    fn creates_lists_saves_and_loads_challenge_solutions() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());

        let (first_name, mut first_grid) =
            files.create_challenge_solution(ChallengeId::Nor).unwrap();
        let (second_name, _) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
        assert_eq!(first_name, "Solution 1");
        assert_eq!(second_name, "Solution 2");

        first_grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        files
            .save_challenge_solution(ChallengeId::Nor, &first_name, &first_grid, true)
            .unwrap();

        let solutions = files.list_challenge_solutions(ChallengeId::Nor).unwrap();
        assert_eq!(
            solutions,
            vec![
                ChallengeSolutionFile {
                    name: "Solution 1".to_owned(),
                    passing: true,
                },
                ChallengeSolutionFile {
                    name: "Solution 2".to_owned(),
                    passing: false,
                },
            ]
        );
        assert_eq!(
            files
                .load_ref(&ComponentFileRef::ChallengeSolution {
                    challenge: ChallengeId::Nor,
                    name: first_name,
                })
                .unwrap()
                .snapshot(),
            first_grid.snapshot()
        );

        remove_test_root(&root);
    }

    #[test]
    fn saves_challenge_progress_independently_from_solution_passing() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (name, grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
        files
            .save_challenge_solution(ChallengeId::Nor, &name, &grid, true)
            .unwrap();
        let mut progress = ChallengeProgress::default();
        progress.passed.insert(ChallengeId::Nor, true);
        files.save_progress(&progress).unwrap();

        files
            .save_challenge_solution(ChallengeId::Nor, &name, &grid, false)
            .unwrap();

        assert!(
            !files
                .list_challenge_solutions(ChallengeId::Nor)
                .unwrap()
                .first()
                .unwrap()
                .passing
        );
        assert!(files.load_progress().unwrap().is_passed(ChallengeId::Nor));

        remove_test_root(&root);
    }

    #[test]
    fn compiles_challenge_solution_as_subcomponent() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (name, mut grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: logicgame::grid::InputId(0),
            },
        );
        grid.add_component(
            Point::new(4, 4),
            Rotation::Down,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: logicgame::grid::OutputId(0),
            },
        );
        files
            .save_challenge_solution(ChallengeId::Nor, &name, &grid, false)
            .unwrap();

        let kind = files
            .compile_subcomponent(&ComponentFileRef::ChallengeSolution {
                challenge: ChallengeId::Nor,
                name,
            })
            .unwrap();
        assert!(matches!(kind, ComponentKind::Subcomponent { .. }));

        remove_test_root(&root);
    }
}
