use super::*;

/// Creates a source component file with a valid body and I/O pins, then compiles
/// it, returning its source ref and compiled kind ready to pin to the hotbar.
fn pinnable(files: &ComponentFiles, name: &str) -> (ComponentFileRef, ComponentKind) {
    let (id, mut grid) = files.create(name).unwrap();
    let source = ComponentFileRef { id };
    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: Scale::new(8).unwrap(),
            output_scale: Scale::new(8).unwrap(),
        },
    );
    grid.add_component(
        Point::new(0, -2),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::new(2).unwrap(),
            id: logicgame::grid::InputId::from_u128(u128::MAX),

            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(4, 8),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::new(4).unwrap(),
            id: logicgame::grid::OutputId::from_u128(u128::MAX),

            label: String::new(),
        },
    );
    files.save(&source, &grid).unwrap();
    let kind = files.compile_subcomponent(&source, "Sub").unwrap();
    (source, kind)
}

#[test]
fn persists_hotbar_entries_in_save_index() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());

    assert!(files.load_hotbar().unwrap().is_empty());

    let (first, first_kind) = pinnable(&files, "First");
    let (_second, second_kind) = pinnable(&files, "Second");

    files.add_hotbar("First", &first_kind).unwrap();
    files.add_hotbar("Second", &second_kind).unwrap();
    // Re-pinning the same source replaces its entry in place, not a duplicate.
    files.add_hotbar("First renamed", &first_kind).unwrap();
    assert_eq!(
        files.load_hotbar().unwrap(),
        vec![
            ("First renamed".to_owned(), first_kind.clone()),
            ("Second".to_owned(), second_kind.clone()),
        ]
    );

    // The pinned set survives reopening the same root, with no recompilation.
    let reopened = ComponentFiles::new(root.clone());
    assert_eq!(
        reopened.load_hotbar().unwrap(),
        vec![
            ("First renamed".to_owned(), first_kind.clone()),
            ("Second".to_owned(), second_kind.clone()),
        ]
    );

    files.remove_hotbar(first).unwrap();
    assert_eq!(
        files.load_hotbar().unwrap(),
        vec![("Second".to_owned(), second_kind.clone())]
    );
    // Removing an unpinned source is a no-op.
    files.remove_hotbar(first).unwrap();
    assert_eq!(
        files.load_hotbar().unwrap(),
        vec![("Second".to_owned(), second_kind)]
    );

    remove_test_root(&root);
}
