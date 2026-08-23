use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rustc-env=BLOCK_APP_COMMIT={}", commit());

    for path in ["../../.git/HEAD", "../../.git/refs/heads"] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

fn commit() -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map_or_else(|| "unknown".to_owned(), |hash| hash.trim().to_owned())
}
