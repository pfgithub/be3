use std::path::{Path, PathBuf};

use paint_snapshot::Snapshot;

const REVIEW: &str =
    "push it to the dev branch and review it in a Paint review block, which reads them from there";

pub fn assert_snapshot(name: &str, snapshot: &Snapshot) {
    let bytes = snapshot
        .encode()
        .expect("the painting could not be encoded");
    let accepted = accepted_path(name);
    let updating = std::env::var_os("UPDATE_SNAPSHOTS").is_some();

    if updating || !accepted.exists() {
        write(&accepted, &bytes);
        if updating {
            return;
        }
        panic!("wrote a new painting to {}, {REVIEW}", accepted.display());
    }

    let previous = Snapshot::decode(&read(&accepted)).expect("the accepted painting is unreadable");
    let Some(difference) = paint_snapshot::difference(&previous, snapshot) else {
        return;
    };

    panic!(
        "the painting changed: {}\nto accept it:\n  UPDATE_SNAPSHOTS=1 cargo nextest run --workspace\nthen {REVIEW}",
        difference.description
    );
}

fn accepted_path(name: &str) -> PathBuf {
    let manifest = manifest();
    directory(&manifest).join(format!("{}.{name}.paint", crate_name(&manifest)))
}

fn manifest() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is not set, run these tests through cargo");
    PathBuf::from(manifest)
}

fn crate_name(manifest: &Path) -> String {
    manifest
        .file_name()
        .unwrap_or_else(|| panic!("{} names no crate", manifest.display()))
        .to_string_lossy()
        .into_owned()
}

fn directory(manifest: &Path) -> PathBuf {
    manifest
        .ancestors()
        .find(|directory| holds_the_workspace(directory))
        .unwrap_or_else(|| {
            panic!(
                "no workspace holds {}, run these tests through cargo",
                manifest.display()
            )
        })
        .join("snapshots")
}

fn holds_the_workspace(directory: &Path) -> bool {
    std::fs::read_to_string(directory.join("Cargo.toml"))
        .is_ok_and(|manifest| manifest.contains("[workspace]"))
}

fn write(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("could not create {}: {error}", parent.display()));
    }
    std::fs::write(path, bytes)
        .unwrap_or_else(|error| panic!("could not write {}: {error}", path.display()));
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}
