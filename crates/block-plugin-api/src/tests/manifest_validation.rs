use super::*;

#[test]
fn manifest_validation() {
    let manifest = PluginManifest {
        identity: PluginIdentity {
            id: "be3.counter".into(),
            name: "Counter".into(),
            version: "1".into(),
        },
        block_type: [1; 16],
        display_name: "Counter".into(),
        icon: "123".into(),
        creation: CreationMode::Immediate,
        children: ChildOperations::default(),
        important: false,
        interaction: InteractionMode::Live,
        capabilities: EditorCapabilities::default(),
        resize: ResizeMode::Both,
        regions: vec![EditorRegion::Main, EditorRegion::Toolbar],
        entry_points: EntryPoints {
            web: Some("/counter.js".into()),
            windows: Some("counter.exe".into()),
        },
        surfaces: vec![
            SurfaceMechanism::WebExternalImage,
            SurfaceMechanism::WindowsDxgi,
        ],
    };
    assert_eq!(manifest.validate(), Ok(()));

    let mut invalid = manifest.clone();
    invalid.regions.push(EditorRegion::Toolbar);
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest.clone();
    invalid.regions = vec![EditorRegion::LeftSidebar];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest;
    invalid.surfaces.clear();
    assert_eq!(invalid.validate(), Err(ManifestError::MissingSurface));
}
