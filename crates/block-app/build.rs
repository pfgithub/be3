use std::env;
use std::path::Path;
use std::process::Command;

/// Stamps the commit the app was built from into the binary, for the About
/// window to show next to the version.
fn main() {
    println!("cargo:rustc-env=BLOCK_APP_COMMIT={}", commit());
    // Without these the hash would only be refreshed when a source file
    // changes. Missing paths are skipped: asking cargo to watch one would make
    // it rerun this script on every build.
    for path in ["../../.git/HEAD", "../../.git/refs/heads"] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    // Only the web build's debug window runs a wasm module, so the nested
    // cross-compile below (and the wasm32-unknown-unknown target it needs) is
    // skipped entirely for every other build.
    if env::var("TARGET").is_ok_and(|target| target.starts_with("wasm32")) {
        println!("cargo:rustc-env=WASM_DEMO_PATH={}", build_wasm_demo());
    }
}

/// Cross-compiles the `wasm-demo` crate to a standalone WebAssembly module and
/// returns the path to the resulting `.wasm` file, so the debug window's web
/// build can embed it with `include_bytes!`.
fn build_wasm_demo() -> String {
    println!("cargo:rerun-if-changed=../wasm-demo/src");
    println!("cargo:rerun-if-changed=../wasm-demo/Cargo.toml");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    // A target directory separate from the outer build's own avoids the two
    // cargo invocations deadlocking over the same lock file.
    let nested_target_dir = Path::new(&out_dir).join("wasm-demo-target");

    let status = Command::new("rustup")
        .args(["target", "add", "wasm32-unknown-unknown"])
        .status();
    if !status.is_ok_and(|status| status.success()) {
        panic!(
            "failed to ensure the wasm32-unknown-unknown target is installed; run \
             `rustup target add wasm32-unknown-unknown` manually"
        );
    }

    let status = Command::new(env::var("CARGO").expect("CARGO is set by cargo"))
        .args([
            "build",
            "--package",
            "wasm-demo",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
        ])
        .arg("--target-dir")
        .arg(&nested_target_dir)
        .current_dir(Path::new(&manifest_dir).join("../wasm-demo"))
        // CARGO_ENCODED_RUSTFLAGS, inherited from the outer build, would
        // otherwise take priority over RUSTFLAGS below and hide it.
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("RUSTFLAGS", "-C link-arg=--allow-undefined")
        .status()
        .expect("failed to run cargo to build wasm-demo");
    if !status.success() {
        panic!("cargo build of wasm-demo for wasm32-unknown-unknown failed");
    }

    nested_target_dir
        .join("wasm32-unknown-unknown/release/wasm_demo.wasm")
        .to_str()
        .expect("the wasm-demo output path is valid UTF-8")
        .to_owned()
}

/// The checked out commit, or `unknown` when building outside a git checkout or
/// without git installed.
fn commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |hash| hash.trim().to_owned())
}
