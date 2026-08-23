use std::{env, path::PathBuf, process::Command};

pub fn wasm() {
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        return;
    }
    let package = env::var("CARGO_PKG_NAME").expect("cargo names the package it is building");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    let target = out
        .ancestors()
        .nth(4)
        .expect("OUT_DIR sits four directories below the target directory")
        .join("game-modules");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());

    let status = Command::new(cargo)
        .args([
            "build",
            "--package",
            &package,
            "--lib",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--target-dir",
        ])
        .arg(&target)
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_BUILD_RUSTFLAGS")
        .env_remove("CARGO_BUILD_TARGET")
        .env_remove("CARGO_MAKEFLAGS")
        .env_remove("RUSTC")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTFLAGS")
        .status()
        .unwrap_or_else(|error| panic!("could not run cargo for {package}: {error}"));
    if !status.success() {
        panic!("building {package} for wasm32-unknown-unknown failed");
    }

    let module = target
        .join("wasm32-unknown-unknown/release")
        .join(format!("{package}.wasm"));
    println!("cargo:rustc-env=GAME_WASM={}", module.display());
}
