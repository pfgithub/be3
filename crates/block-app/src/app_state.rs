use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::AppStateStore;
#[cfg(target_arch = "wasm32")]
pub use web::AppStateStore;

/// Why a read or write of the stored app state failed. The backing store
/// differs by platform, so the reason is already rendered to text here.
#[derive(Clone, Debug)]
pub struct AppStateError(String);

impl fmt::Display for AppStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for AppStateError {}

impl From<String> for AppStateError {
    fn from(message: String) -> Self {
        Self(message)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<rusqlite::Error> for AppStateError {
    fn from(error: rusqlite::Error) -> Self {
        Self(error.to_string())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum ServerLocation {
    Local,
    Remote(String),
}

impl ServerLocation {
    pub fn key(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Remote(url) => url,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct SavedAccount {
    pub server: ServerLocation,
    pub id: Uuid,
    pub email: String,
    pub name: String,
    /// The session token proving this is really the account it claims to be.
    /// The password itself is never stored.
    pub token: String,
    pub last_workspace_id: Option<Uuid>,
}

// The store is backed by SQLite on native and by browser storage on the web,
// so its tests only build where there is a file to open.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
