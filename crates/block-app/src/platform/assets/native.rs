use std::path::Path;

pub(super) fn read(name: &str) -> Result<Vec<u8>, String> {
    let root = std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(Path::to_path_buf))
        .ok_or("the app has no directory of its own")?;
    let path = root.join(name);
    std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
}
