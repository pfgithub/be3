#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> eframe::Result {
    std::env::set_var("RUST_BACKTRACE", "1");
    block_app_lib::run()
}

// Android starts at `android_main` and the browser at `run_web`, both in the
// library; neither has a binary to run.
#[cfg(any(target_os = "android", target_arch = "wasm32"))]
fn main() {}
