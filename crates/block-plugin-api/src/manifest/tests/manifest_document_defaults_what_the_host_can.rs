use super::*;

#[test]
fn manifest_document_defaults_what_the_host_can() {
    let document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    assert_eq!(document.children, ChildOperations::default());
    assert!(!document.important);
    assert_eq!(document.interaction, InteractionMode::default());
    assert_eq!(document.capabilities, EditorCapabilities::default());
    assert_eq!(document.resize, ResizeMode::default());
}
