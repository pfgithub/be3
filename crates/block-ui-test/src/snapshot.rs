use std::path::{Path, PathBuf};

use paint_snapshot::Snapshot;

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
        panic!(
            "wrote a new painting to {}, look at it with:\n  {}",
            accepted.display(),
            render_command(&accepted, name)
        );
    }

    let previous = Snapshot::decode(&read(&accepted)).expect("the accepted painting is unreadable");
    let Some(difference) = paint_snapshot::difference(&previous, snapshot) else {
        return;
    };

    let proposed = accepted.with_extension("new.paint");
    write(&proposed, &bytes);
    panic!(
        "the painting changed: {}\nto look at it:\n  {}\nto accept it:\n  UPDATE_SNAPSHOTS=1 cargo nextest run --workspace",
        difference.description,
        diff_command(&accepted, &proposed, name)
    );
}

fn render_command(accepted: &Path, name: &str) -> String {
    format!(
        "cargo run -p paint-snapshot -- render {} {}",
        accepted.display(),
        output_path(name).display()
    )
}

fn diff_command(accepted: &Path, proposed: &Path, name: &str) -> String {
    format!(
        "cargo run -p paint-snapshot -- diff {} {} {}",
        accepted.display(),
        proposed.display(),
        output_path(name).display()
    )
}

fn output_path(name: &str) -> PathBuf {
    directory().join(format!("{name}.png"))
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
