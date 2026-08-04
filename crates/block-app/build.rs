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
