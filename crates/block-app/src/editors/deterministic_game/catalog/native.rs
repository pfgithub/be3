use std::path::{Path, PathBuf};

use super::Installed;

pub(super) fn scan() -> Installed {
    let mut installed = Installed::default();
    for root in roots() {
        scan_root(&mut installed, &root);
    }
    installed
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    {
        roots.push(directory.join("games"));
    }
    if let Some(storage) = eframe::storage_dir(crate::APP_ID) {
        roots.push(storage.join("games"));
    }
    roots.dedup();
    roots
}

fn scan_root(installed: &mut Installed, root: &Path) {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            installed.error(&root.display().to_string(), error);
            return;
        }
    };
    let mut modules: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "wasm")
        })
        .collect();
    modules.sort();
    for module in modules {
        let source = module.display().to_string();
        let Some(id) = module.file_stem().and_then(|stem| stem.to_str()) else {
            installed.error(&source, "this module's name is not text");
            continue;
        };
        let id = id.to_owned();
        match std::fs::read(&module) {
            Ok(bytes) => installed.add(&source, &id, &bytes),
            Err(error) => installed.error(&source, error),
        }
    }
}
