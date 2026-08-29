use super::*;

#[test]
fn an_empty_entry_point_is_rejected() {
    let source = DOCUMENT.replace("counter.wasm", "");
    assert_eq!(
        manifest_from_json(&source),
        Err(ManifestError::Empty("entry point"))
    );
}
