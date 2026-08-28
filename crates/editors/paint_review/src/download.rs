use std::sync::mpsc::Receiver;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_editor_plugin::Waker;
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use native::start as start_download;
#[cfg(target_arch = "wasm32")]
use web::start as start_download;

pub const REPOSITORY: &str = "pfgithub/be3";
pub const BRANCH: &str = "dev";
pub const FOLDER: &str = "snapshots";

#[derive(Clone)]
pub struct Painting {
    pub path: String,
    pub hash: String,
    pub data: Vec<u8>,
}

#[derive(Clone, Default)]
pub enum Source {
    #[default]
    Branch,
    #[cfg(test)]
    Fixed(Arc<Mutex<Vec<Painting>>>),
}

pub struct Download {
    receiver: Receiver<Result<Vec<Painting>, String>>,
}

impl Download {
    pub fn poll(&mut self) -> Option<Result<Vec<Painting>, String>> {
        self.receiver.try_recv().ok()
    }

    #[cfg(test)]
    fn ready(result: Result<Vec<Painting>, String>) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let _ = sender.send(result);
        Self { receiver }
    }
}

pub fn start(source: &Source, waker: Waker) -> Download {
    match source {
        Source::Branch => start_download(waker),
        #[cfg(test)]
        Source::Fixed(paintings) => Download::ready(Ok(paintings.lock().unwrap().clone())),
    }
}

fn tree_url() -> String {
    format!("https://api.github.com/repos/{REPOSITORY}/git/trees/{BRANCH}:{FOLDER}")
}

fn file_url(path: &str) -> String {
    format!("https://raw.githubusercontent.com/{REPOSITORY}/{BRANCH}/{FOLDER}/{path}")
}

fn painting(path: String, data: Vec<u8>) -> Painting {
    Painting {
        hash: PaintSnapshot::fingerprint(&data),
        path,
        data,
    }
}

pub(crate) fn paths_in(tree: &[u8]) -> Result<Vec<String>, String> {
    let tree: Value = serde_json::from_slice(tree)
        .map_err(|error| format!("GitHub answered with something unreadable: {error}"))?;
    if let Some(message) = tree.get("message").and_then(Value::as_str) {
        return Err(format!(
            "GitHub refused to list {FOLDER} on {BRANCH}: {message}"
        ));
    }
    if tree.get("truncated").and_then(Value::as_bool) == Some(true) {
        return Err(format!(
            "{FOLDER} holds more paintings than GitHub will list at once"
        ));
    }
    let entries = tree
        .get("tree")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("GitHub sent no listing of {FOLDER}"))?;
    let suffix = format!(".{}", PaintSnapshot::FILE_EXTENSION);
    let mut paths: Vec<String> = entries
        .iter()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("blob"))
        .filter_map(|entry| entry.get("path").and_then(Value::as_str))
        .filter(|path| path.ends_with(&suffix))
        .map(str::to_owned)
        .collect();
    paths.sort();
    Ok(paths)
}
