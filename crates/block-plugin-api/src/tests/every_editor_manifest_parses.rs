use super::*;

#[test]
fn every_editor_manifest_parses() {
    let editors = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../editors");
    let mut checked = 0;
    for entry in std::fs::read_dir(&editors).expect("the editors are beside this crate") {
        let manifest = entry
            .expect("the entry is readable")
            .path()
            .join("manifest.json");
        if !manifest.exists() {
            continue;
        }
        let document = std::fs::read_to_string(&manifest).expect("the manifest is readable");
        manifest_from_json(&document)
            .unwrap_or_else(|error| panic!("{}: {error}", manifest.display()));
        checked += 1;
    }
    assert!(checked > 0, "no editor manifests were found");
}
