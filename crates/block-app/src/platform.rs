//! The parts of the app that depend on where it runs.
//!
//! Desktop and Android have threads, a filesystem to keep app state in, and can
//! host the block server in-process. The browser has none of those: work that
//! would go to a thread runs as a task on the event loop instead, and the app
//! talks to a remote server only.

use std::sync::mpsc::Receiver;

mod file_picker;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use file_picker::{FileFilter, FilePicker, PickedFile};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{spawn_request, start_embedded_server, EmbeddedServer};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::spawn_request;

/// Whether this build can run a block server inside the app. The browser cannot
/// listen for connections, so the web build only reaches a remote server.
pub(crate) const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

/// The result of a request started with [`spawn_request`], polled from the UI
/// each frame so drawing never waits on the network.
pub(crate) type RequestResult<T> = Receiver<T>;
