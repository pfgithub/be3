use super::*;
use std::io;

#[test]
fn deletes_components_from_save_index() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (id, mut grid) = files.create("Sub").unwrap();
    let file = ComponentFileRef { id };

    grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::MergerSplitter {
            input_scale: Scale::new(8).unwrap(),
            output_scale: Scale::new(8).unwrap(),
        },
    );
    grid.add_component(
        Point::new(0, -1),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: logicgame::grid::InputId::from_u128(1),
            label: String::new(),
        },
    );
    grid.add_component(
        Point::new(4, 8),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: logicgame::grid::OutputId::from_u128(1),
            label: String::new(),
        },
    );
    files.save(&file, &grid).unwrap();
    let kind = files.compile_subcomponent(&file, "Sub").unwrap();
    files.add_hotbar("Sub", &kind).unwrap();

    files.delete(&file).unwrap();

    assert!(files.list().unwrap().is_empty());
    assert!(matches!(
        files.load_ref(&file),
        Err(ComponentFileError::Io(error)) if error.kind() == io::ErrorKind::NotFound
    ));
    assert!(files.load_hotbar().unwrap().is_empty());

    remove_test_root(&root);
}
