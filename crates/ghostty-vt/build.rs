use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn main() {
    let target = env::var("TARGET").expect("cargo sets TARGET");
    let manifest = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let repository = manifest
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives in crates/ghostty-vt");
    let script = repository.join("scripts/internal/build-ghostty-vt.sh");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-env-changed=GHOSTTY_VT_LIBRARY_DIRECTORY");

    let directory = match env::var_os("GHOSTTY_VT_LIBRARY_DIRECTORY") {
        Some(directory) => PathBuf::from(directory),
        None => {
            build(&script, &target);
            repository.join("target/ghostty-vt").join(&target)
        }
    };

    let library = if target.ends_with("windows-msvc") {
        "ghostty-vt-static"
    } else {
        "ghostty-vt"
    };
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static={library}");
}

fn build(script: &Path, target: &str) {
    match Command::new("bash")
        .arg(script)
        .arg("--triple")
        .arg(target)
        .stdout(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("{} failed with status {status}", script.display()),
        Err(error) => panic!(
            "failed to run {} with bash: {error}. Install bash and Zig, or build the archive \
             separately and point GHOSTTY_VT_LIBRARY_DIRECTORY at the directory holding it.",
            script.display()
        ),
    }
}
