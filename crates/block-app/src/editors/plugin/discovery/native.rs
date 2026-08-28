use std::path::{Path, PathBuf};

use super::Plugins;

const SUFFIX: &str = ".plugin.json";

pub(super) fn scan() -> Plugins {
    let mut plugins = Plugins::default();
    let Some(root) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    else {
        return plugins;
    };
    for manifest in manifests(&mut plugins, &root) {
        let source = manifest.display().to_string();
        match std::fs::read_to_string(&manifest) {
            Ok(document) => plugins.add(&source, &document),
            Err(error) => plugins.error(&source, error),
        }
    }
    plugins.root = root;
    plugins
}

fn manifests(plugins: &mut Plugins, root: &Path) -> Vec<PathBuf> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(error) => {
            plugins.error(&root.display().to_string(), error);
            return Vec::new();
        }
    };
    let mut manifests: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(SUFFIX))
        })
        .collect();
    manifests.sort();
    manifests
}
