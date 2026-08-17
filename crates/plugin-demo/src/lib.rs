#[cfg(target_arch = "wasm32")]
mod app;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;
#[cfg(target_os = "windows")]
pub mod windows_surface;
