mod block_data;
mod client;
mod network;
pub(crate) mod plugins;
pub(crate) mod version;
// libghostty-vt is not supported on Windows, and is disabled on macOS because
// it does not currently build there.
#[cfg(not(any(
    target_os = "android",
    target_os = "windows",
    target_os = "macos",
    target_arch = "wasm32"
)))]
pub(crate) mod terminal;
