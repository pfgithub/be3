use super::*;

#[test]
fn manifest_document_rejects_unknown_fields() {
    let source = DOCUMENT.replace("\"icon\"", "\"ikon\"");
    assert!(matches!(
        ManifestDocument::parse(&source),
        Err(ManifestError::Malformed(_))
    ));
}
