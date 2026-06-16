use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use logicgame::{
    execution::{GenerationError, Instruction, Vm},
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
    #[serde(with = "compiled_vm")]
    pub vm: Vm,
}

mod compiled_vm {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Instruction, Vm};

    #[derive(Serialize)]
    struct SerializedVm<'a> {
        memory: usize,
        storage: &'a [u64],
        inputs: &'a [usize],
        outputs: &'a [usize],
        instructions: &'a [Instruction],
    }

    #[derive(Deserialize)]
    struct DeserializedVm {
        memory: usize,
        storage: Vec<u64>,
        inputs: Vec<usize>,
        outputs: Vec<usize>,
        instructions: Vec<Instruction>,
    }

    pub fn serialize<S>(vm: &Vm, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        SerializedVm {
            memory: vm.memory.len(),
            storage: &vm.storage,
            inputs: &vm.inputs,
            outputs: &vm.outputs,
            instructions: &vm.instructions,
        }
        .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vm, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vm = DeserializedVm::deserialize(deserializer)?;
        Ok(Vm {
            memory: vec![0; vm.memory],
            storage: vm.storage,
            inputs: vm.inputs,
            outputs: vm.outputs,
            instructions: vm.instructions,
            components: Vec::new(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct ComponentFileDrag {
    pub name: String,
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
                    if !entry.file_type()?.is_file()
                        || entry.path().extension().and_then(|value| value.to_str()) != Some("json")
                    {
                        continue;
                    }
                    if let Some(name) = entry.path().file_stem().and_then(|value| value.to_str()) {
                        if validate_name(name).is_ok() {
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

    pub fn load(&self, name: &str) -> Result<LogicGrid, ComponentFileError> {
        let name = validate_name(name)?;
        let bytes = fs::read(self.path(name))?;
        let snapshot = serde_json::from_slice::<LogicGridSnapshot>(&bytes)?;
        Ok(LogicGrid::from_snapshot(snapshot))
    }

    pub fn save(&self, name: &str, grid: &LogicGrid) -> Result<(), ComponentFileError> {
        let name = validate_name(name)?;
        fs::create_dir_all(&self.root)?;
        let bytes = serde_json::to_vec_pretty(&grid.snapshot())?;
        atomic_write(&self.path(name), &bytes)?;
        Ok(())
    }

    pub fn compile_subcomponent(&self, name: &str) -> Result<ComponentKind, ComponentFileError> {
        let grid = self.load(name)?;
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
        let vm = Vm::from_graph(&grid, &grid.generate_graph())?;
        let bytes = serde_json::to_vec_pretty(&CompiledComponent { snapshot, vm })?;
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
        vm.load_components(|hash| {
            let bytes = fs::read(self.compiled_path(hash))?;
            Ok(serde_json::from_slice::<CompiledComponent>(&bytes)?.vm)
        })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.json"))
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

    use logicgame::grid::{ComponentKind, ComponentSide, Point, Rotation, Scale};

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
    fn compiled_vm_serializes_io_bindings() {
        let compiled = CompiledComponent {
            snapshot: LogicGrid::new().snapshot(),
            vm: Vm {
                memory: vec![3, 4, 5],
                storage: vec![6, 7],
                inputs: vec![2],
                outputs: vec![1],
                instructions: Vec::new(),
                components: Vec::new(),
            },
        };

        let json = serde_json::to_value(&compiled).unwrap();
        assert_eq!(json["vm"]["memory"], 3);
        assert_eq!(json["vm"]["storage"], serde_json::json!([6, 7]));
        assert_eq!(json["vm"]["inputs"], serde_json::json!([2]));
        assert_eq!(json["vm"]["outputs"], serde_json::json!([1]));
        assert!(json["vm"]["instructions"].is_array());

        let decoded: CompiledComponent = serde_json::from_value(json).unwrap();
        assert_eq!(decoded.snapshot, compiled.snapshot);
        assert_eq!(decoded.vm.memory, vec![0; 3]);
        assert_eq!(decoded.vm.storage, compiled.vm.storage);
        assert_eq!(decoded.vm.inputs, compiled.vm.inputs);
        assert_eq!(decoded.vm.outputs, compiled.vm.outputs);
        assert_eq!(decoded.vm.instructions, compiled.vm.instructions);
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

        let first = files.compile_subcomponent("source").unwrap();
        let second = files.compile_subcomponent("source").unwrap();
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
        assert_eq!(compiled.vm.inputs.len(), 1);
        assert_eq!(compiled.vm.outputs.len(), 1);
        assert_eq!(component.as_str(), format!("{:x}", Sha256::digest(&bytes)));

        grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
        files.save("source", &grid).unwrap();
        let changed = files.compile_subcomponent("source").unwrap();
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
            files.compile_subcomponent("empty"),
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
            files.compile_subcomponent("internal-port"),
            Err(ComponentFileError::InvalidSubcomponent(_))
        ));

        remove_test_root(&root);
    }
}
