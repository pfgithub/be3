#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result {
    block_app_lib::run()
}

#[cfg(target_os = "android")]
fn main() {}
