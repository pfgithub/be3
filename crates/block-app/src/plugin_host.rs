#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod input;
#[cfg(target_os = "windows")]
mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod presenter;
#[cfg(target_os = "windows")]
mod process;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "android")]
pub(crate) use android::{close, editor_ui};
#[cfg(target_os = "windows")]
pub(crate) use native::{close, editor_ui, install};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{close, editor_ui, install};
