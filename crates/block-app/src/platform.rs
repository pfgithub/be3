use std::sync::mpsc::Receiver;

mod file_picker;
pub(crate) mod http;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

pub(crate) use file_picker::{FileFilter, FilePicker};
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{spawn_request, start_embedded_server, EmbeddedServer};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::spawn_request;

pub(crate) const HAS_EMBEDDED_SERVER: bool = cfg!(not(target_arch = "wasm32"));

pub(crate) type RequestResult<T> = Receiver<T>;
