use std::path::Path;

use super::Installed;

const INDEX: &str = "games.json";

pub(super) fn scan() -> Installed {
    let mut installed = Installed::default();
    let Some(root) = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
    else {
        return installed;
    };
    let index = root.join(INDEX);
    let source = index.display().to_string();
    let document = match std::fs::read(&index) {
        Ok(document) => document,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return installed,
        Err(error) => {
            installed.error(&source, error);
            return installed;
        }
    };
    let modules = match serde_json::from_slice::<Vec<String>>(&document) {
        Ok(modules) => modules,
        Err(error) => {
            installed.error(&source, error);
            return installed;
        }
    };
    for module in modules {
        let path = root.join(&module);
        let source = path.display().to_string();
        let Some(id) = path.file_stem().and_then(|stem| stem.to_str()) else {
            installed.error(&source, "this module's name is not text");
            continue;
        };
        let id = id.to_owned();
        match std::fs::read(&path) {
            Ok(bytes) => installed.add(&source, &id, &bytes),
            Err(error) => installed.error(&source, error),
        }
    }
    installed
}
