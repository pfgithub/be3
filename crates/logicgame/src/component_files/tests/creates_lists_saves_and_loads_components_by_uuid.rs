use super::*;

#[test]
fn creates_lists_saves_and_loads_components_by_uuid() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (zed_id, mut grid) = files.create("Zed").unwrap();
    let (alpha_id, _) = files.create("alpha").unwrap();
    fs::write(root.join("notes.txt"), b"ignored").unwrap();

    let listed = files.list().unwrap();
    assert_eq!(
        listed,
        vec![
            ComponentFile {
                id: alpha_id,
                name: "alpha".to_owned(),
                completed: false,
            },
            ComponentFile {
                id: zed_id,
                name: "Zed".to_owned(),
                completed: false,
            },
        ]
    );
    assert!(root.join(format!("{zed_id}.json")).exists());
    assert!(root
        .parent()
        .expect("test root has parent")
        .join("save.json")
        .exists());
    assert!(matches!(
        files.create("Zed"),
        Err(ComponentFileError::AlreadyExists(_))
    ));

    grid.add_component(
        Point::new(2, 4),
        Rotation::Right,
        ComponentKind::Not {
            scale: Scale::new(2).unwrap(),
        },
    );
    let file = ComponentFileRef { id: zed_id };
    files.save(&file, &grid).unwrap();
    assert_eq!(files.load_ref(&file).unwrap().snapshot(), grid.snapshot());

    remove_test_root(&root);
}
