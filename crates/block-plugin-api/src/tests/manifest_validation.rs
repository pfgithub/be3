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
        regions: vec![EditorRegion::Frame, EditorRegion::Preview],
        chrome: vec![EditorBand::Toolbar],
        entry_point: "counter.wasm".into(),
        network: vec!["api.github.com".into()],
    };
    assert_eq!(manifest.validate(), Ok(()));

    let mut invalid = manifest.clone();
    invalid.regions.push(EditorRegion::Preview);
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest.clone();
    invalid.regions = vec![EditorRegion::Preview];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidRegions));

    let mut invalid = manifest.clone();
    invalid.chrome.push(EditorBand::Toolbar);
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidChrome));

    let mut invalid = manifest.clone();
    invalid.entry_point = String::new();
    assert_eq!(invalid.validate(), Err(ManifestError::Empty("entry point")));

    let mut invalid = manifest.clone();
    invalid.network = vec!["https://api.github.com/".into()];
    assert_eq!(invalid.validate(), Err(ManifestError::InvalidNetworkHost));

    let mut invalid = manifest;
    invalid.network = vec![String::new()];
    assert_eq!(
        invalid.validate(),
        Err(ManifestError::Empty("network host"))
    );
}
