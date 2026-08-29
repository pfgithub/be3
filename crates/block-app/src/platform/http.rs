pub(crate) const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::Fetch;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::Fetch;
