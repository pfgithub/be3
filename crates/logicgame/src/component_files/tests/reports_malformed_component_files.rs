use super::*;

#[test]
fn reports_malformed_component_files() {
    let root = test_root();
    fs::create_dir_all(&root).unwrap();
    let files = ComponentFiles::new(root.clone());
    let (id, _) = files.create("broken").unwrap();
    fs::write(root.join(format!("{id}.json")), b"{").unwrap();
    assert!(matches!(
        files.load_ref(&ComponentFileRef { id }),
        Err(ComponentFileError::Json(_))
    ));
    remove_test_root(&root);
}
