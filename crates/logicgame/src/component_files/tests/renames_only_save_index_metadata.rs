use super::*;

#[test]
fn renames_only_save_index_metadata() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, mut grid) = files.create("Before").unwrap();
    grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    let file = ComponentFileRef { id };
    files.save(&file, &grid).unwrap();
    let path = root.join(format!("{id}.json"));
    let before = fs::read(&path).unwrap();

    files
        .rename(&ComponentFileSource::Component, &file, "After")
        .unwrap();

    assert_eq!(fs::read(&path).unwrap(), before);
    assert_eq!(files.list().unwrap()[0].name, "After");
    assert_eq!(files.load_ref(&file).unwrap().snapshot(), grid.snapshot());

    remove_test_root(&root);
}
