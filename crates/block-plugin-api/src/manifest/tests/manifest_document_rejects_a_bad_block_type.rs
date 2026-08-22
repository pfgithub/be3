use super::*;

#[test]
fn manifest_document_rejects_a_bad_block_type() {
    let mut document = ManifestDocument::parse(DOCUMENT).expect("the document is valid");
    document.block_type = "not a uuid".into();
    assert_eq!(
        document.into_manifest(),
        Err(ManifestError::InvalidBlockType)
    );
}
