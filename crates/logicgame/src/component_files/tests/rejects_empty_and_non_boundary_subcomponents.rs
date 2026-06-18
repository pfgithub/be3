use super::*;

#[test]
fn rejects_empty_and_non_boundary_subcomponents() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (empty, _) = files.create("empty").unwrap();
    assert!(matches!(
        files.compile_subcomponent(&ComponentFileRef { id: empty }, "Sub"),
        Err(ComponentFileError::InvalidSubcomponent(_))
    ));

    remove_test_root(&root);
}
