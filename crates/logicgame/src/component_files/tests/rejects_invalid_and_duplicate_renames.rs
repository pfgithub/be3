use super::*;

#[test]
fn rejects_invalid_and_duplicate_renames() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (first, _) = files.create("First").unwrap();
    files.create("Second").unwrap();

    assert!(matches!(
        files.rename(
            &ComponentFileSource::Component,
            &ComponentFileRef { id: first },
            "Second"
        ),
        Err(ComponentFileError::AlreadyExists(_))
    ));
    assert!(matches!(
        files.rename(
            &ComponentFileSource::Component,
            &ComponentFileRef { id: first },
            "../escape"
        ),
        Err(ComponentFileError::InvalidName(_))
    ));

    remove_test_root(&root);
}
