use super::*;

#[test]
fn manifest_from_json_reads_a_document() {
    let manifest = manifest_from_json(DOCUMENT).expect("the document is valid");
    assert_eq!(manifest.identity.id, "be3.counter");
    assert_eq!(manifest.identity.name, "Counter");
    assert_eq!(manifest.identity.version, "0.1.0");
    assert_eq!(
        manifest.block_type,
        Uuid::parse_str("636f756e-7465-722d-626c-6f636b2d0001")
            .expect("a uuid")
            .into_bytes()
    );
    assert_eq!(manifest.icon, "\u{eb8d}");
    assert_eq!(manifest.creation, CreationMode::Immediate);
    assert_eq!(
        manifest.regions,
        vec![EditorRegion::Main, EditorRegion::Toolbar]
    );
    assert_eq!(manifest.entry_points.wasm.as_deref(), Some("counter.wasm"));
    assert_eq!(manifest.entry_points.windows, None);
    assert_eq!(manifest.entry_points.linux, None);
}
