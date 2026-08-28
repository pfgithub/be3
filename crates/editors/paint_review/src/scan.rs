use std::path::{Path, PathBuf};

use block_client::blocks::paint_snapshot::PaintSnapshot;

const SKIPPED: &[&str] = &["target", "node_modules"];

pub struct Painting {
    pub path: String,
    pub hash: String,
    pub data: Vec<u8>,
}

#[cfg(target_arch = "wasm32")]
pub fn root() -> Result<PathBuf, String> {
    Err("Paintings can only be reviewed in the desktop app".to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn root() -> Result<PathBuf, String> {
    std::env::current_dir()
        .map_err(|error| format!("could not find the working directory: {error}"))
}

pub fn scan(root: &Path) -> Result<Vec<Painting>, String> {
    std::fs::read_dir(root)
        .map_err(|error| format!("could not read {}: {error}", root.display()))?;
    let mut found = Vec::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir() {
                if !name.starts_with('.') && !SKIPPED.contains(&name.as_str()) {
                    directories.push(path);
                }
            } else if name.ends_with(&format!(".{}", PaintSnapshot::FILE_EXTENSION)) {
                found.push(painting(root, &path)?);
            }
        }
    }
    found.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(found)
}

fn painting(root: &Path, path: &Path) -> Result<Painting, String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    Ok(Painting {
        path: relative(root, path),
        hash: PaintSnapshot::fingerprint(&data),
        data,
    })
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
