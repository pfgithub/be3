#[cfg(target_os = "android")]
mod android;
#[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "android"))]
mod input;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod presenter;
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
mod process;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "android")]
pub(crate) use android::{install, open, show};
#[cfg(all(not(target_arch = "wasm32"), not(target_os = "android")))]
pub(crate) use native::{install, open, show};
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{install, open, show};
