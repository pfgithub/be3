fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    paint_review::run();
}
