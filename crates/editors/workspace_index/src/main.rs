fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    workspace_index::run();
}
