#[cfg(target_arch = "wasm32")]
mod app;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod demo;
#[cfg(target_os = "windows")]
mod egui_session;

pub mod native;
#[cfg(target_os = "windows")]
pub mod windows_surface;
