#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(super) use native::Fetch;
#[cfg(target_arch = "wasm32")]
pub(super) use web::Fetch;
