use super::*;

#[test]
fn a_wasm_entry_point_needs_a_host_texture_surface() {
    let source = r#"{
        "id": "be3.demo",
        "name": "Demo",
        "version": "1.0",
        "block_type": "636f756e-7465-722d-626c-6f636b2d0001",
        "display_name": "Demo",
        "icon": "x",
        "creation": "Immediate",
        "regions": ["Main"],
        "entry_points": { "wasm": "demo.wasm" },
        "surfaces": ["LinuxDmaBuf"]
    }"#;
    assert_eq!(
        manifest_from_json(source),
        Err(ManifestError::MissingSurface)
    );
    let accepted = source.replace("LinuxDmaBuf", "HostTexture");
    let manifest = manifest_from_json(&accepted).expect("the manifest should be accepted");
    assert_eq!(manifest.entry_points.wasm.as_deref(), Some("demo.wasm"));
}
