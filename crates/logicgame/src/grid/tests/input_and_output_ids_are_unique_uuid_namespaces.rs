use super::*;

#[test]
fn input_and_output_ids_are_unique_uuid_namespaces() {
    let mut grid = LogicGrid::new();
    let first_input = grid.add_component(
        Point::new(0, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(99),

            label: String::new(),
        },
    );
    let first_output = grid.add_component(
        Point::new(2, 0),
        Rotation::Up,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(99),

            label: String::new(),
        },
    );
    grid.remove_component(first_input);
    let second_input = grid.add_component(
        Point::new(4, 0),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(99),

            label: String::new(),
        },
    );

    let ComponentKind::Output {
        id: first_output_id,
        ..
    } = grid.component(first_output).unwrap().kind
    else {
        panic!("expected output");
    };
    let ComponentKind::Input {
        id: second_input_id,
        ..
    } = grid.component(second_input).unwrap().kind
    else {
        panic!("expected input");
    };
    assert_ne!(first_output_id.0, second_input_id.0);
    assert_ne!(first_output_id, OutputId::from_u128(99));
    assert_ne!(second_input_id, InputId::from_u128(99));
}
