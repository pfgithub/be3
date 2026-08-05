mod client;
mod network;
// libghostty-vt is not supported on Windows.
#[cfg(not(any(target_os = "android", target_os = "windows", target_arch = "wasm32")))]
pub(crate) mod terminal;
