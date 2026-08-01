#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    block_app::run()
}

#[cfg(target_os = "android")]
fn main() {}
