mod block_data;
mod client;
mod network;
pub(crate) mod plugins;
pub(crate) mod version;

#[cfg(not(any(
    target_os = "android",
    target_os = "windows",
    target_os = "macos",
    target_arch = "wasm32"
)))]
pub(crate) mod terminal;
