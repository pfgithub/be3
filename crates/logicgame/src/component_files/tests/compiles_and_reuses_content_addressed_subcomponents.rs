use super::*;

#[test]
fn compiles_and_reuses_content_addressed_subcomponents() {
    let root = test_root();
    let files = ComponentFiles::new(root.clone());
    let (source_id, mut grid) = files.create("source").unwrap();
    let source = ComponentFileRef { id: source_id };
    // The body defines the bounds; Input and Output pins sit flush against its
    // edges and are excluded from the bounds.
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

    let first = files.compile_subcomponent(&source, "Sub").unwrap();
    let second = files.compile_subcomponent(&source, "Sub").unwrap();
    assert_eq!(first, second);
    let ComponentKind::Subcomponent {
        component,
        size,
        ports,
        ..
    } = first
    else {
        panic!("expected subcomponent");
    };
    assert_eq!(size, Size::new(8, 8));
    assert_eq!(
        ports,
        vec![
            ComponentPort::input(0, Scale::new(2).unwrap(), ComponentSide::Top, 0, 2),
            ComponentPort::output(0, Scale::new(4).unwrap(), ComponentSide::Bottom, 4, 8),
        ]
    );
    let path = files.compiled_path(&component);
    let bytes = fs::read(&path).unwrap();
    let compiled: CompiledComponent = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(compiled.source_file_id, source_id);
    assert_eq!(compiled.snapshot, grid.snapshot());
    assert_eq!(compiled.component.inputs.len(), 1);
    assert_eq!(compiled.component.outputs.len(), 1);
    assert_eq!(component.as_str(), format!("{:x}", Sha256::digest(&bytes)));

    grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
    files.save(&source, &grid).unwrap();
    let changed = files.compile_subcomponent(&source, "Sub").unwrap();
    let ComponentKind::Subcomponent {
        component: changed_hash,
        ..
    } = changed
    else {
        panic!("expected subcomponent");
    };
    assert_ne!(changed_hash, component);
    assert!(files.compiled_path(&changed_hash).exists());

    remove_test_root(&root);
}
