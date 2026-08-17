#[cfg(target_arch = "wasm32")]
mod input;
#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod presenter;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::{install, open, show};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{install, open, show};
