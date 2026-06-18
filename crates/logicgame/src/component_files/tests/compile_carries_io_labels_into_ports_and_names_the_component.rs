use super::*;

#[test]
fn compile_carries_io_labels_into_ports_and_names_the_component() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (source_id, mut grid) = files.create("source").unwrap();
    let source = ComponentFileRef { id: source_id };
    // A body that defines the bounds, with a labelled input and output flush
    // against its edges.
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
            label: "A".to_owned(),
        },
    );
    grid.add_component(
        Point::new(4, 8),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::new(4).unwrap(),
            id: logicgame::grid::OutputId::from_u128(u128::MAX),
            label: "SUM".to_owned(),
        },
    );
    files.save(&source, &grid).unwrap();

    let kind = files.compile_subcomponent(&source, "Half Adder").unwrap();
    let ComponentKind::Subcomponent { name, ports, .. } = kind else {
        panic!("expected subcomponent");
    };

    assert_eq!(name, "Half Adder");
    let labels: Vec<&str> = ports.iter().map(|port| port.label.as_str()).collect();
    assert_eq!(labels, vec!["A", "SUM"]);

    remove_test_root(&root);
}
