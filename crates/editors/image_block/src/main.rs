fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    image_block::run();
}
