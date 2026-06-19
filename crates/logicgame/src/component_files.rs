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
    /// User-customized hotbar slots. The compiled `ComponentKind` for custom
    /// components is stored as-is so it is not recompiled on load; compilation
    /// happens only when a component is first added.
    #[serde(default)]
    hotbar: Vec<SaveHotbarSlot>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveHotbarSlot {
    Builtin {
        tool: String,
    },
    Locked {
        name: String,
    },
    ChallengeCompletion {
        challenge: ChallengeId,
    },
    Folder {
        name: String,
        slots: Vec<SaveHotbarSlot>,
    },
    Custom {
        name: String,
        kind: ComponentKind,
    },
}

impl Default for SaveIndex {
    fn default() -> Self {
        let challenges = logicgame::challenges::CHALLENGES
            .into_iter()
            .map(|challenge| (challenge, SaveChallenge::default()))
            .collect();
        Self {
            component_files: Vec::new(),
            challenges,
            hotbar: Vec::new(),
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
        let challenge_name = challenge.name();
        let name = loop {
            let name = format!("{challenge_name} {candidate}");
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

    pub fn delete(&self, file: &ComponentFileRef) -> Result<(), ComponentFileError> {
        let mut index = self.load_index()?;
        let before = index.component_files.len();
        index.component_files.retain(|saved| saved.id != file.id);
        if index.component_files.len() == before {
            return Err(ComponentFileError::InvalidSubcomponent(
                "Component metadata is missing",
            ));
        }
        remove_saved_hotbar_source(&mut index.hotbar, *file);
        self.save_index(&index)?;
        match fs::remove_file(self.path(file.id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
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

    pub fn delete_challenge_solution(
        &self,
        challenge: ChallengeId,
        id: Uuid,
    ) -> Result<(), ComponentFileError> {
        let mut index = self.load_index()?;
        let challenge_index = index.challenges.entry(challenge).or_default();
        let before = challenge_index.files.len();
        challenge_index.files.retain(|file| file.id != id);
        if challenge_index.files.len() == before {
            return Err(ComponentFileError::InvalidSubcomponent(
                "Challenge solution metadata is missing",
            ));
        }
        remove_saved_hotbar_source(&mut index.hotbar, ComponentFileRef { id });
        self.save_index(&index)?;
        match fs::remove_file(self.path(id)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
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
        name: &str,
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
        let ports = subcomponent_ports(&grid, bounds.min)?;
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

        let mut kind =
            ComponentKind::subcomponent_with_subgraphs(*file, hash, size, ports, subgraphs)?;
        if let ComponentKind::Subcomponent { name: target, .. } = &mut kind {
            *target = name.to_owned();
        }
        Ok(kind)
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

    /// The saved hotbar tree. Custom component kinds are returned verbatim from
    /// the save index without recompiling.
    pub fn load_hotbar(&self) -> Result<Vec<SaveHotbarSlot>, ComponentFileError> {
        Ok(self.load_index()?.hotbar)
    }

    pub fn save_hotbar(&self, hotbar: Vec<SaveHotbarSlot>) -> Result<(), ComponentFileError> {
        let mut index = self.load_index()?;
        index.hotbar = hotbar;
        self.save_index(&index)
    }

    /// Pins an already-compiled component to the hotbar. If the same source file
    /// is already pinned its entry is replaced in place rather than duplicated.
    /// Non-subcomponent kinds are ignored.
    pub fn add_hotbar(&self, name: &str, kind: &ComponentKind) -> Result<(), ComponentFileError> {
        let Some(source) = hotbar_entry_source(kind) else {
            return Ok(());
        };
        let mut index = self.load_index()?;
        let entry = SaveHotbarSlot::Custom {
            name: name.to_owned(),
            kind: kind.clone(),
        };
        if let Some(existing) = find_saved_hotbar_source_mut(&mut index.hotbar, source) {
            *existing = entry;
        } else {
            index.hotbar.push(entry);
        }
        self.save_index(&index)
    }

    /// Unpins the hotbar entry for a source component file. No-op if not pinned.
    pub fn remove_hotbar(&self, source: ComponentFileRef) -> Result<(), ComponentFileError> {
        let mut index = self.load_index()?;
        if remove_saved_hotbar_source(&mut index.hotbar, source) {
            self.save_index(&index)?;
        }
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

fn hotbar_entry_source(kind: &ComponentKind) -> Option<ComponentFileRef> {
    match kind {
        ComponentKind::Subcomponent { source_file_id, .. } => Some(*source_file_id),
        _ => None,
    }
}

fn find_saved_hotbar_source_mut(
    slots: &mut [SaveHotbarSlot],
    source: ComponentFileRef,
) -> Option<&mut SaveHotbarSlot> {
    for slot in slots {
        match slot {
            SaveHotbarSlot::Custom { kind, .. } if hotbar_entry_source(kind) == Some(source) => {
                return Some(slot);
            }
            SaveHotbarSlot::Folder { slots, .. } => {
                if let Some(found) = find_saved_hotbar_source_mut(slots, source) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn remove_saved_hotbar_source(slots: &mut Vec<SaveHotbarSlot>, source: ComponentFileRef) -> bool {
    let before = slots.len();
    slots.retain(|slot| {
        !matches!(
            slot,
            SaveHotbarSlot::Custom { kind, .. } if hotbar_entry_source(kind) == Some(source)
        )
    });
    let mut removed = slots.len() != before;
    for slot in slots {
        if let SaveHotbarSlot::Folder { slots, .. } = slot {
            removed |= remove_saved_hotbar_source(slots, source);
        }
    }
    removed
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
) -> Result<Vec<ComponentPort>, ComponentFileError> {
    let input_indices = dense_input_indices(grid);
    let output_indices = dense_output_indices(grid);
    let mut ports = Vec::new();
    for component in grid.components() {
        let (direction, index, scale, label) = match &component.kind {
            ComponentKind::Input { id, scale, label } => {
                let index = input_indices[id];
                (
                    logicgame::grid::ConnectionDirection::Input,
                    index,
                    *scale,
                    label.clone(),
                )
            }
            ComponentKind::Output { id, scale, label } => {
                let index = output_indices[id];
                (
                    logicgame::grid::ConnectionDirection::Output,
                    index,
                    *scale,
                    label.clone(),
                )
            }
            _ => continue,
        };
        let lead = component
            .lead()
            .ok_or(ComponentFileError::InvalidSubcomponent(
                "Input or output component has no external lead",
            ))?;
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
            label,
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
mod tests;
