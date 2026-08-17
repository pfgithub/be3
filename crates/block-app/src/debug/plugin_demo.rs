#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod input;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod presenter;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod process;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{install, open, show};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{install, open, show};
