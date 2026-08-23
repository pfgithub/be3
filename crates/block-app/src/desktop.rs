#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> eframe::Result {
    std::env::set_var("RUST_BACKTRACE", "1");
    block_app_lib::run()
}

#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {}
