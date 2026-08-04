use logicgame::grid::{
    ComponentKind, ConnectionDirection, InputId, LogicGrid as Grid, OutputId, Point, Rotation,
    Scale,
};

use super::*;

#[test]
fn compiling_a_grid_derives_ports_from_its_inputs_and_outputs() {
    let mut grid = Grid::new();
    // An inverter with its input at the bottom and its output at the top.
    let not = grid.add_component(
        Point::new(0, 1),
        Rotation::Up,
        ComponentKind::Not { scale: Scale::ONE },
    );
    grid.add_component_with_explicit_io(
        Point::new(0, 3),
        Rotation::Up,
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(1),
            label: "IN".to_owned(),
        },
    );
    grid.add_component_with_explicit_io(
        Point::new(0, 0),
        Rotation::Down,
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(1),
            label: "OUT".to_owned(),
        },
    );
    assert!(grid.component(not).is_some());

    let source = Uuid::new_v4();
    let program = CompiledLogic::compile(source, &grid).unwrap();

    assert_eq!(program.source(), source);
    assert_eq!(
        program
            .ports()
            .iter()
            .map(|port| (port.direction, port.label.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (ConnectionDirection::Input, "IN"),
            (ConnectionDirection::Output, "OUT"),
        ]
    );
    // Nothing else is called, so the compiled block references nothing.
    assert!(program.calls().is_empty());
}
