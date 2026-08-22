use std::path::{Path, PathBuf};

use super::Plugins;

pub(super) fn scan() -> Plugins {
    let mut plugins = Plugins::default();
    for root in roots() {
        scan_root(&mut plugins, &root);
    }
    plugins
}

fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(directory) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    {
        roots.push(directory.join("plugins"));
    }
    if let Some(storage) = eframe::storage_dir(crate::APP_ID) {
        roots.push(storage.join("plugins"));
    }
    roots.dedup();
    roots
}

fn scan_root(plugins: &mut Plugins, root: &Path) {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return,
        Err(error) => {
            plugins.error(&root.display().to_string(), error);
            return;
        }
    };
    let mut directories: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    directories.sort();
    for directory in directories {
        let manifest = directory.join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let source = manifest.display().to_string();
        match std::fs::read_to_string(&manifest) {
            Ok(document) => plugins.add(&source, directory, &document),
            Err(error) => plugins.error(&source, error),
        }
    }
}
