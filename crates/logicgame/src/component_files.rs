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
        ComponentFileRef, ComponentHash, ComponentKind, ComponentPort, ComponentSide,
        ComponentSubgraph, GeometryError, LogicGrid, LogicGridSnapshot, Point, Size,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

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
    pub source_file_id: Uuid,
    pub snapshot: LogicGridSnapshot,
    pub component: UnlinkedComponent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ComponentFileSource {
    Component,
    ChallengeSolution { challenge: ChallengeId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentFileDrag {
    pub file: ComponentFileRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComponentFile {
    pub id: Uuid,
    pub name: String,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeSolutionFile {
    pub id: Uuid,
    pub name: String,
    pub completed: bool,
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
struct SaveIndex {
    component_files: Vec<SaveFile>,
    challenges: BTreeMap<ChallengeId, SaveChallenge>,
}

impl Default for SaveIndex {
    fn default() -> Self {
        let challenges = logicgame::challenges::CHALLENGES
            .into_iter()
            .map(|challenge| (challenge.id, SaveChallenge::default()))
            .collect();
        Self {
            component_files: Vec::new(),
            challenges,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct SaveChallenge {
    completed: bool,
    files: Vec<SaveFile>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SaveFile {
    id: Uuid,
    name: String,
    completed: bool,
}

#[derive(Clone)]
pub struct ComponentFiles {
    root: PathBuf,
    save_path: PathBuf,
    compiled_root: PathBuf,
}

impl ComponentFiles {
    pub fn new(root: PathBuf) -> Self {
        let save_path = root.parent().map_or_else(
            || PathBuf::from("save.json"),
            |parent| parent.join("save.json"),
        );
        let compiled_root = root.parent().map_or_else(
            || PathBuf::from("compiled"),
            |parent| parent.join("compiled"),
        );
        Self {
            root,
            save_path,
            compiled_root,
        }
    }

    pub fn list(&self) -> Result<Vec<ComponentFile>, ComponentFileError> {
        let mut files: Vec<_> = self
            .load_index()?
            .component_files
            .into_iter()
            .map(|file| ComponentFile {
                id: file.id,
                name: file.name,
                completed: file.completed,
            })
            .collect();
        files.sort_by_key(|file| file.name.to_lowercase());
        Ok(files)
    }

    pub fn list_challenge_solutions(
        &self,
        challenge: ChallengeId,
    ) -> Result<Vec<ChallengeSolutionFile>, ComponentFileError> {
        let mut solutions: Vec<_> = self
            .load_index()?
            .challenges
            .remove(&challenge)
            .unwrap_or_default()
            .files
            .into_iter()
            .map(|file| ChallengeSolutionFile {
                id: file.id,
                name: file.name,
                completed: file.completed,
            })
            .collect();
        solutions.sort_by_key(|solution| solution.name.to_lowercase());
        Ok(solutions)
    }

    pub fn create(&self, name: &str) -> Result<(Uuid, LogicGrid), ComponentFileError> {
        let name = validate_name(name)?.to_owned();
        let mut index = self.load_index()?;
        reject_duplicate(&index.component_files, Uuid::nil(), &name)?;

        let id = Uuid::new_v4();
        let grid = LogicGrid::new();
        self.save_grid(id, &grid)?;
        index.component_files.push(SaveFile {
            id,
            name,
            completed: false,
        });
        self.save_index(&index)?;
        Ok((id, grid))
    }

    pub fn create_challenge_solution(
        &self,
        challenge: ChallengeId,
    ) -> Result<(Uuid, String, LogicGrid), ComponentFileError> {
        let mut index = self.load_index()?;
        let challenge_index = index.challenges.entry(challenge).or_default();
        let mut candidate = 1;
        let name = loop {
            let name = format!("Solution {candidate}");
            if !challenge_index.files.iter().any(|file| file.name == name) {
                break name;
            }
            candidate += 1;
        };

        let id = Uuid::new_v4();
        let grid = LogicGrid::new();
        self.save_grid(id, &grid)?;
        challenge_index.files.push(SaveFile {
            id,
            name: name.clone(),
            completed: false,
        });
        self.save_index(&index)?;
        Ok((id, name, grid))
    }

    pub fn load_ref(&self, file: &ComponentFileRef) -> Result<LogicGrid, ComponentFileError> {
        Ok(LogicGrid::from_snapshot(self.load_snapshot(file.id)?))
    }

    pub fn save(
        &self,
        file: &ComponentFileRef,
        grid: &LogicGrid,
    ) -> Result<(), ComponentFileError> {
        self.save_grid(file.id, grid)
    }

    pub fn save_challenge_solution(
        &self,
        challenge: ChallengeId,
        id: Uuid,
        grid: &LogicGrid,
        completed: bool,
    ) -> Result<(), ComponentFileError> {
        self.save_grid(id, grid)?;
        let mut index = self.load_index()?;
        let challenge_index = index.challenges.entry(challenge).or_default();
        let file = challenge_index
            .files
            .iter_mut()
            .find(|file| file.id == id)
            .ok_or(ComponentFileError::InvalidSubcomponent(
                "Challenge solution metadata is missing",
            ))?;
        file.completed = completed;
        self.save_index(&index)
    }

    pub fn rename(
        &self,
        source: &ComponentFileSource,
        file: &ComponentFileRef,
        new_name: &str,
    ) -> Result<(), ComponentFileError> {
        let new_name = validate_name(new_name)?.to_owned();
        let mut index = self.load_index()?;
        let id = &file.id;
        match source {
            ComponentFileSource::Component => {
                reject_duplicate(&index.component_files, *id, &new_name)?;
                let file = index
                    .component_files
                    .iter_mut()
                    .find(|file| file.id == *id)
                    .ok_or(ComponentFileError::InvalidSubcomponent(
                        "Component metadata is missing",
                    ))?;
                file.name = new_name;
            }
            ComponentFileSource::ChallengeSolution { challenge } => {
                let challenge_index = index.challenges.entry(*challenge).or_default();
                reject_duplicate(&challenge_index.files, *id, &new_name)?;
                let file = challenge_index
                    .files
                    .iter_mut()
                    .find(|file| file.id == *id)
                    .ok_or(ComponentFileError::InvalidSubcomponent(
                        "Challenge solution metadata is missing",
                    ))?;
                file.name = new_name;
            }
        }
        self.save_index(&index)
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
        let subgraphs = component
            .subgraphs
            .iter()
            .map(|subgraph| ComponentSubgraph {
                inputs: subgraph.inputs.clone(),
                outputs: subgraph.outputs.clone(),
            })
            .collect();
        let bytes = serde_json::to_vec_pretty(&CompiledComponent {
            source_file_id: file.id,
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

        Ok(ComponentKind::subcomponent_with_subgraphs(
            *file, hash, size, ports, subgraphs,
        )?)
    }

    fn compiled_path(&self, hash: &ComponentHash) -> PathBuf {
        self.compiled_root.join(format!("{hash}.json"))
    }

    pub fn load_components(&self, vm: &mut Vm) -> Result<(), ComponentFileError> {
        let mut cache = BTreeMap::<ComponentHash, Rc<Component>>::new();
        vm.load_components(|hash| self.load_component(hash, &mut cache))
    }

    pub fn load_progress(&self) -> Result<ChallengeProgress, ComponentFileError> {
        let passed = self
            .load_index()?
            .challenges
            .into_iter()
            .map(|(id, challenge)| (id, challenge.completed))
            .collect();
        Ok(ChallengeProgress { passed })
    }

    pub fn save_progress(&self, progress: &ChallengeProgress) -> Result<(), ComponentFileError> {
        let mut index = self.load_index()?;
        for (id, completed) in &progress.passed {
            index.challenges.entry(*id).or_default().completed = *completed;
        }
        self.save_index(&index)
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

    fn load_index(&self) -> Result<SaveIndex, ComponentFileError> {
        match fs::read(&self.save_path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(SaveIndex::default()),
            Err(error) => Err(error.into()),
        }
    }

    fn save_index(&self, index: &SaveIndex) -> Result<(), ComponentFileError> {
        if let Some(parent) = self.save_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(index)?;
        atomic_write(&self.save_path, &bytes)?;
        Ok(())
    }

    fn load_snapshot(&self, id: Uuid) -> Result<LogicGridSnapshot, ComponentFileError> {
        let bytes = fs::read(self.path(id))?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    fn save_grid(&self, id: Uuid, grid: &LogicGrid) -> Result<(), ComponentFileError> {
        fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec_pretty(&grid.snapshot())?;
        atomic_write(&self.path(id), &bytes)?;
        Ok(())
    }

    fn path(&self, id: Uuid) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }
}

fn reject_duplicate(
    files: &[SaveFile],
    current: Uuid,
    name: &str,
) -> Result<(), ComponentFileError> {
    if files
        .iter()
        .any(|file| file.id != current && file.name == name)
    {
        return Err(ComponentFileError::AlreadyExists(name.to_owned()));
    }
    Ok(())
}

fn subcomponent_ports(
    grid: &LogicGrid,
    min: Point,
    max: Point,
) -> Result<Vec<ComponentPort>, ComponentFileError> {
    let input_indices = dense_input_indices(grid);
    let output_indices = dense_output_indices(grid);
    let mut ports = Vec::new();
    for component in grid.components() {
        let (direction, index, scale) = match component.kind {
            ComponentKind::Input { id, scale } => {
                let index = input_indices[&id];
                (logicgame::grid::ConnectionDirection::Input, index, scale)
            }
            ComponentKind::Output { id, scale } => {
                let index = output_indices[&id];
                (logicgame::grid::ConnectionDirection::Output, index, scale)
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

fn dense_input_indices(grid: &LogicGrid) -> BTreeMap<logicgame::grid::InputId, usize> {
    dense_port_indices(
        grid.components()
            .filter_map(|component| match component.kind {
                ComponentKind::Input { id, .. } => Some(id),
                _ => None,
            }),
    )
}

fn dense_output_indices(grid: &LogicGrid) -> BTreeMap<logicgame::grid::OutputId, usize> {
    dense_port_indices(
        grid.components()
            .filter_map(|component| match component.kind {
                ComponentKind::Output { id, .. } => Some(id),
                _ => None,
            }),
    )
}

fn dense_port_indices<T: Ord>(ids: impl IntoIterator<Item = T>) -> BTreeMap<T, usize> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect()
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
    fn creates_lists_saves_and_loads_components_by_uuid() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (zed_id, mut grid) = files.create("Zed").unwrap();
        let (alpha_id, _) = files.create("alpha").unwrap();
        fs::write(root.join("notes.txt"), b"ignored").unwrap();

        let listed = files.list().unwrap();
        assert_eq!(
            listed,
            vec![
                ComponentFile {
                    id: alpha_id,
                    name: "alpha".to_owned(),
                    completed: false,
                },
                ComponentFile {
                    id: zed_id,
                    name: "Zed".to_owned(),
                    completed: false,
                },
            ]
        );
        assert!(root.join(format!("{zed_id}.json")).exists());
        assert!(root
            .parent()
            .expect("test root has parent")
            .join("save.json")
            .exists());
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
        let file = ComponentFileRef { id: zed_id };
        files.save(&file, &grid).unwrap();
        assert_eq!(files.load_ref(&file).unwrap().snapshot(), grid.snapshot());

        remove_test_root(&root);
    }

    #[test]
    fn reports_malformed_component_files() {
        let root = test_root();
        fs::create_dir_all(&root).unwrap();
        let files = ComponentFiles::new(root.clone());
        let (id, _) = files.create("broken").unwrap();
        fs::write(root.join(format!("{id}.json")), b"{").unwrap();
        assert!(matches!(
            files.load_ref(&ComponentFileRef { id }),
            Err(ComponentFileError::Json(_))
        ));
        remove_test_root(&root);
    }

    #[test]
    fn renames_only_save_index_metadata() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (id, mut grid) = files.create("Before").unwrap();
        grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        let file = ComponentFileRef { id };
        files.save(&file, &grid).unwrap();
        let path = root.join(format!("{id}.json"));
        let before = fs::read(&path).unwrap();

        files
            .rename(&ComponentFileSource::Component, &file, "After")
            .unwrap();

        assert_eq!(fs::read(&path).unwrap(), before);
        assert_eq!(files.list().unwrap()[0].name, "After");
        assert_eq!(files.load_ref(&file).unwrap().snapshot(), grid.snapshot());

        remove_test_root(&root);
    }

    #[test]
    fn rejects_invalid_and_duplicate_renames() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (first, _) = files.create("First").unwrap();
        files.create("Second").unwrap();

        assert!(matches!(
            files.rename(
                &ComponentFileSource::Component,
                &ComponentFileRef { id: first },
                "Second"
            ),
            Err(ComponentFileError::AlreadyExists(_))
        ));
        assert!(matches!(
            files.rename(
                &ComponentFileSource::Component,
                &ComponentFileRef { id: first },
                "../escape"
            ),
            Err(ComponentFileError::InvalidName(_))
        ));

        remove_test_root(&root);
    }

    #[test]
    fn compiled_component_serializes_unlinked_component_and_source_id() {
        let source_file_id = Uuid::new_v4();
        let compiled = CompiledComponent {
            source_file_id,
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
                subgraphs: vec![logicgame::execution::UnlinkedSubgraph {
                    inputs: vec![0],
                    outputs: vec![0],
                    instructions: vec![Instruction::Not {
                        input: 2,
                        output: 1,
                    }],
                }],
            },
        };

        let json = serde_json::to_value(&compiled).unwrap();
        assert_eq!(json["source_file_id"], source_file_id.to_string());
        assert_eq!(json["component"]["memory_size"], 3);
        assert_eq!(json["component"]["storage_init"], serde_json::json!([6, 7]));
        assert_eq!(json["component"]["inputs"], serde_json::json!([2]));
        assert_eq!(json["component"]["outputs"], serde_json::json!([1]));
        assert!(json["component"]["instructions"].is_array());
        assert!(json["component"]["subgraphs"].is_array());

        let decoded: CompiledComponent = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.source_file_id, compiled.source_file_id);
        assert_eq!(decoded.snapshot, compiled.snapshot);
        assert_eq!(decoded.component, compiled.component);
    }

    #[test]
    fn compiles_and_reuses_content_addressed_subcomponents() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (source_id, mut grid) = files.create("source").unwrap();
        let source = ComponentFileRef { id: source_id };
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::new(2).unwrap(),
                id: logicgame::grid::InputId::from_u128(u128::MAX),
            },
        );
        grid.add_component(
            Point::new(4, 4),
            Rotation::Down,
            ComponentKind::Output {
                scale: Scale::new(4).unwrap(),
                id: logicgame::grid::OutputId::from_u128(u128::MAX),
            },
        );
        files.save(&source, &grid).unwrap();

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
        assert_eq!(compiled.source_file_id, source_id);
        assert_eq!(compiled.snapshot, grid.snapshot());
        assert_eq!(compiled.component.inputs.len(), 1);
        assert_eq!(compiled.component.outputs.len(), 1);
        assert_eq!(component.as_str(), format!("{:x}", Sha256::digest(&bytes)));

        grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
        files.save(&source, &grid).unwrap();
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
        let (empty, _) = files.create("empty").unwrap();
        assert!(matches!(
            files.compile_subcomponent(&ComponentFileRef { id: empty }),
            Err(ComponentFileError::InvalidSubcomponent(_))
        ));

        let (internal_port, mut grid) = files.create("internal-port").unwrap();
        let file = ComponentFileRef { id: internal_port };
        grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        grid.add_component(
            Point::new(4, 4),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: logicgame::grid::InputId::from_u128(u128::MAX),
            },
        );
        grid.add_component(Point::new(4, -4), Rotation::Up, ComponentKind::Led);
        files.save(&file, &grid).unwrap();
        assert!(matches!(
            files.compile_subcomponent(&file),
            Err(ComponentFileError::InvalidSubcomponent(_))
        ));

        remove_test_root(&root);
    }

    #[test]
    fn creates_lists_saves_and_loads_challenge_solutions_from_save_index() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());

        let (first_id, first_name, mut first_grid) =
            files.create_challenge_solution(ChallengeId::Nor).unwrap();
        let (second_id, second_name, _) =
            files.create_challenge_solution(ChallengeId::Nor).unwrap();
        assert_eq!(first_name, "Solution 1");
        assert_eq!(second_name, "Solution 2");

        first_grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        files
            .save_challenge_solution(ChallengeId::Nor, first_id, &first_grid, true)
            .unwrap();
        fs::write(root.join(format!("{second_id}.json")), b"{").unwrap();

        let solutions = files.list_challenge_solutions(ChallengeId::Nor).unwrap();
        assert_eq!(
            solutions,
            vec![
                ChallengeSolutionFile {
                    id: first_id,
                    name: "Solution 1".to_owned(),
                    completed: true,
                },
                ChallengeSolutionFile {
                    id: second_id,
                    name: "Solution 2".to_owned(),
                    completed: false,
                },
            ]
        );
        assert_eq!(
            files
                .load_ref(&ComponentFileRef { id: first_id })
                .unwrap()
                .snapshot(),
            first_grid.snapshot()
        );

        remove_test_root(&root);
    }

    #[test]
    fn saves_challenge_progress_in_save_index_independently_from_solution_completion() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (id, _, grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
        files
            .save_challenge_solution(ChallengeId::Nor, id, &grid, true)
            .unwrap();
        let mut progress = ChallengeProgress::default();
        progress.passed.insert(ChallengeId::Nor, true);
        files.save_progress(&progress).unwrap();

        files
            .save_challenge_solution(ChallengeId::Nor, id, &grid, false)
            .unwrap();

        assert!(
            !files
                .list_challenge_solutions(ChallengeId::Nor)
                .unwrap()
                .first()
                .unwrap()
                .completed
        );
        assert!(files.load_progress().unwrap().is_passed(ChallengeId::Nor));

        remove_test_root(&root);
    }

    #[test]
    fn compiles_challenge_solution_as_subcomponent() {
        let root = test_root();
        let files = ComponentFiles::new(root.clone());
        let (id, _, mut grid) = files.create_challenge_solution(ChallengeId::Nor).unwrap();
        let file = ComponentFileRef { id };
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: logicgame::grid::InputId::from_u128(1),
            },
        );
        grid.add_component(
            Point::new(4, 4),
            Rotation::Down,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: logicgame::grid::OutputId::from_u128(1),
            },
        );
        files
            .save_challenge_solution(ChallengeId::Nor, id, &grid, false)
            .unwrap();

        let kind = files.compile_subcomponent(&file).unwrap();
        assert!(matches!(kind, ComponentKind::Subcomponent { .. }));

        remove_test_root(&root);
    }
}
