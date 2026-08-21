use std::{
    fs,
    sync::mpsc::{self, Receiver},
};

use super::{FileFilter, PickResult, PickedFile};

/// The desktop dialog is modal, so it has already closed by the time this
/// returns and the answer is waiting on the first poll.
pub(super) fn open(filter: &FileFilter) -> Receiver<PickResult> {
    let (sender, receiver) = mpsc::channel();
    let _ = sender.send(pick(filter));
    receiver
}

fn pick(filter: &FileFilter) -> PickResult {
    let extensions: Vec<&str> = filter.extensions.iter().map(String::as_str).collect();
    let Some(path) = rfd::FileDialog::new()
        .add_filter(&filter.name, &extensions)
        .pick_file()
    else {
        return Ok(None);
    };
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or_default()
        .to_owned();
    let data =
        fs::read(&path).map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    Ok(Some(PickedFile { name, data }))
}
