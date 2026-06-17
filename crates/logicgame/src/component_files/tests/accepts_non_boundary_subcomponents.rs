use super::*;

#[test]
fn accepts_non_boundary_subcomponents() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());

    let (internal_port, mut grid) = files.create("internal-port").unwrap();
    let file = ComponentFileRef { id: internal_port };
    grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    grid.add_component(
        Point::new(4, 4),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: logicgame::grid::InputId::from_u128(u128::MAX),
        },
    );
    grid.add_component(Point::new(4, -4), Rotation::Up, ComponentKind::Led);
    files.save(&file, &grid).unwrap();
    assert!(matches!(files.compile_subcomponent(&file), Ok(_)));

    remove_test_root(&root);
}
