use std::env;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").expect("cargo sets TARGET");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let repository = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives in crates/ghostty-vt");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=GHOSTTY_VT_LIBRARY_DIRECTORY");

    let directory = match env::var_os("GHOSTTY_VT_LIBRARY_DIRECTORY") {
        Some(directory) => PathBuf::from(directory),
        None => repository.join("target/ghostty-vt").join(&target),
    };

    let (library, file) = if target.ends_with("windows-msvc") {
        ("ghostty-vt-static", "ghostty-vt-static.lib")
    } else {
        ("ghostty-vt", "libghostty-vt.a")
    };

    let archive = directory.join(file);
    if !archive.is_file() {
        panic!(
            "{} does not exist. It is built by scripts/internal/build-ghostty-vt.sh, which \
             ./scripts/build and ./scripts/verify run for you; run one of those instead of cargo \
             directly, run the script yourself with --triple {target}, or point \
             GHOSTTY_VT_LIBRARY_DIRECTORY at a directory holding the archive.",
            archive.display()
        );
    }

    println!("cargo:rerun-if-changed={}", archive.display());
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static={library}");
}
