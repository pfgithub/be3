use std::{
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use logicgame::grid::{LogicGrid, LogicGridSnapshot};

static TEMP_FILE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum ComponentFileError {
    InvalidName(&'static str),
    AlreadyExists(String),
    Io(io::Error),
    Json(serde_json::Error),
}

impl fmt::Display for ComponentFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(message) => formatter.write_str(message),
            Self::AlreadyExists(name) => write!(formatter, "A component named \"{name}\" exists"),
            Self::Io(error) => write!(formatter, "File error: {error}"),
            Self::Json(error) => write!(formatter, "Invalid component file: {error}"),
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

pub struct ComponentFiles {
    root: PathBuf,
}

impl ComponentFiles {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
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

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.json"))
    }
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

    use logicgame::grid::{ComponentKind, Point, Rotation, Scale};

    use super::*;

    static TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn test_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "logicgame-components-{}-{}",
            std::process::id(),
            TEST_ID.fetch_add(1, Ordering::Relaxed)
        ))
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

        fs::remove_dir_all(root).unwrap();
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
        fs::remove_dir_all(root).unwrap();
    }
}
