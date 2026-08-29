use std::collections::VecDeque;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use block_client::blocks::paint_snapshot::PaintSnapshot;
use block_editor_plugin::{EditorHost, FetchResult};
use serde_json::Value;

const AT_ONCE: usize = 8;

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

struct Wanted {
    request: u64,
    path: String,
}

struct Files {
    queued: VecDeque<String>,
    wanted: Vec<Wanted>,
    found: Vec<Painting>,
}

enum Stage {
    Tree(u64),
    Files(Files),
    Finished,
    #[cfg(test)]
    Fixed(Vec<Painting>),
}

pub struct Download {
    stage: Stage,
}

impl Download {
    pub fn poll(&mut self, host: &EditorHost) -> Option<Result<Vec<Painting>, String>> {
        let found = self.advance(host)?;
        self.stage = Stage::Finished;
        Some(found)
    }

    fn advance(&mut self, host: &EditorHost) -> Option<Result<Vec<Painting>, String>> {
        match &mut self.stage {
            Stage::Finished => return None,
            #[cfg(test)]
            Stage::Fixed(paintings) => return Some(Ok(std::mem::take(paintings))),
            Stage::Tree(request) => {
                let listing = match host.take_fetch(*request)? {
                    FetchResult::Body(listing) => listing,
                    FetchResult::Failed(error) => return Some(Err(error)),
                };
                let paths = match paths_in(&listing) {
                    Ok(paths) => paths,
                    Err(error) => return Some(Err(error)),
                };
                self.stage = Stage::Files(Files {
                    queued: paths.into(),
                    wanted: Vec::new(),
                    found: Vec::new(),
                });
            }
            Stage::Files(..) => {}
        }
        let Stage::Files(Files {
            queued,
            wanted,
            found,
        }) = &mut self.stage
        else {
            return None;
        };
        let mut failure = None;
        wanted.retain(|painting| match host.take_fetch(painting.request) {
            None => true,
            Some(FetchResult::Body(data)) => {
                found.push(read(painting.path.clone(), data));
                false
            }
            Some(FetchResult::Failed(error)) => {
                failure = Some(error);
                false
            }
        });
        if let Some(error) = failure {
            return Some(Err(error));
        }
        while wanted.len() < AT_ONCE {
            let Some(path) = queued.pop_front() else {
                break;
            };
            wanted.push(Wanted {
                request: host.fetch(file_url(&path)),
                path,
            });
        }
        if !wanted.is_empty() {
            return None;
        }
        let mut found = std::mem::take(found);
        found.sort_by(|left, right| left.path.cmp(&right.path));
        Some(Ok(found))
    }
}

pub fn start(source: &Source, host: &EditorHost) -> Download {
    let stage = match source {
        Source::Branch => Stage::Tree(host.fetch(tree_url())),
        #[cfg(test)]
        Source::Fixed(paintings) => Stage::Fixed(paintings.lock().unwrap().clone()),
    };
    Download { stage }
}

fn tree_url() -> String {
    format!("https://api.github.com/repos/{REPOSITORY}/git/trees/{BRANCH}:{FOLDER}")
}

fn file_url(path: &str) -> String {
    format!("https://raw.githubusercontent.com/{REPOSITORY}/{BRANCH}/{FOLDER}/{path}")
}

fn read(path: String, data: Vec<u8>) -> Painting {
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
