use std::{cell::RefCell, sync::Arc};

use game_host::Game;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::load;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub(crate) use android::load;

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod native;

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
pub(crate) struct Installed {
    games: Vec<Entry>,
    errors: Vec<String>,
}

impl Installed {
    pub(crate) fn games(&self) -> &[Entry] {
        &self.games
    }

    pub(crate) fn errors(&self) -> &[String] {
        &self.errors
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

thread_local! {
    static INSTALLED: RefCell<Option<Arc<Installed>>> = const { RefCell::new(None) };
}

#[cfg_attr(
    all(not(target_arch = "wasm32"), not(target_os = "android")),
    allow(dead_code)
)]
fn install(installed: Installed) {
    INSTALLED.with(|cell| *cell.borrow_mut() = Some(Arc::new(installed)));
}

pub(crate) fn installed() -> Arc<Installed> {
    INSTALLED.with(|cell| {
        let mut cell = cell.borrow_mut();
        Arc::clone(cell.get_or_insert_with(|| Arc::new(scan())))
    })
}

#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
fn scan() -> Installed {
    native::scan()
}

#[cfg(any(target_arch = "wasm32", target_os = "android"))]
fn scan() -> Installed {
    Installed::default()
}

pub(crate) fn game(id: &str) -> Option<Arc<Game>> {
    installed()
        .games
        .iter()
        .find(|entry| entry.id == id)
        .map(|entry| Arc::clone(&entry.game))
}
