use std::path::{Path, PathBuf};

use paint_snapshot::Snapshot;

const REVIEW: &str =
    "review it in a Paint review block, in the app run from the root of the repository";

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
    directory().join(format!("{name}.paint"))
}

fn directory() -> PathBuf {
    let manifest = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR is not set, run these tests through cargo");
    PathBuf::from(manifest).join("snapshots")
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
