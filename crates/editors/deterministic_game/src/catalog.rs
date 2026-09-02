use std::collections::HashMap;
use std::sync::Arc;

use block_editor_plugin::{AssetResult, EditorHost};
use game_host::Game;

const INDEX: &str = "games.json";

pub(crate) struct Entry {
    id: String,
    game: Arc<Game>,
}

impl Entry {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn display_name(&self) -> &str {
        self.game.name()
    }
}

#[derive(Default)]
pub(crate) struct Catalog {
    stage: Stage,
    games: Vec<Entry>,
    errors: Vec<String>,
}

#[derive(Default)]
enum Stage {
    #[default]
    Unread,
    Index(u64),
    Modules(HashMap<u64, String>),
    Installed,
}

impl Catalog {
    pub(crate) fn poll(&mut self, host: &EditorHost) {
        match std::mem::take(&mut self.stage) {
            Stage::Unread => self.stage = Stage::Index(host.read_asset(INDEX)),
            Stage::Index(request) => self.read_index(host, request),
            Stage::Modules(pending) => self.read_modules(host, pending),
            Stage::Installed => self.stage = Stage::Installed,
        }
    }

    pub(crate) fn installed(&self) -> bool {
        matches!(self.stage, Stage::Installed)
    }

    pub(crate) fn games(&self) -> &[Entry] {
        &self.games
    }

    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
    }

    pub(crate) fn game(&self, id: &str) -> Option<Arc<Game>> {
        self.games
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| Arc::clone(&entry.game))
    }

    fn read_index(&mut self, host: &EditorHost, request: u64) {
        let body = match host.take_asset(request) {
            Some(AssetResult::Body(body)) => body,
            Some(AssetResult::Failed(error)) => {
                self.error(INDEX, error);
                self.stage = Stage::Installed;
                return;
            }
            None => {
                self.stage = Stage::Index(request);
                return;
            }
        };
        let modules = match serde_json::from_slice::<Vec<String>>(&body) {
            Ok(modules) => modules,
            Err(error) => {
                self.error(INDEX, error);
                self.stage = Stage::Installed;
                return;
            }
        };
        let pending: HashMap<u64, String> = modules
            .into_iter()
            .map(|module| (host.read_asset(&module), module))
            .collect();
        self.stage = match pending.is_empty() {
            true => Stage::Installed,
            false => Stage::Modules(pending),
        };
    }

    fn read_modules(&mut self, host: &EditorHost, mut pending: HashMap<u64, String>) {
        pending.retain(|request, module| match host.take_asset(*request) {
            Some(AssetResult::Body(bytes)) => {
                self.add(module, &identify(module), &bytes);
                false
            }
            Some(AssetResult::Failed(error)) => {
                self.errors.push(format!("{module}: {error}"));
                false
            }
            None => true,
        });
        self.stage = match pending.is_empty() {
            true => {
                self.games.sort_by(|left, right| left.id.cmp(&right.id));
                Stage::Installed
            }
            false => Stage::Modules(pending),
        };
    }

    fn add(&mut self, source: &str, id: &str, module: &[u8]) {
        if self.games.iter().any(|entry| entry.id == id) {
            self.errors
                .push(format!("{source}: {id} is already an installed game"));
            return;
        }
        match Game::load(module) {
            Ok(game) => self.games.push(Entry {
                id: id.to_owned(),
                game: Arc::new(game),
            }),
            Err(error) => self.error(source, error),
        }
    }

    fn error(&mut self, source: &str, error: impl std::fmt::Display) {
        self.errors.push(format!("{source}: {error}"));
    }
}

fn identify(module: &str) -> String {
    let name = module.rsplit('/').next().unwrap_or(module);
    name.strip_suffix(".wasm").unwrap_or(name).to_owned()
}
