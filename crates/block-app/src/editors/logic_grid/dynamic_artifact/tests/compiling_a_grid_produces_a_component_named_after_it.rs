use block::Block;
use block_client::blocks::logic_grid::LogicGridOperation;
use logicgame::grid::{
    Component, ComponentKind, ComponentOrientation, InputId, OutputId, Point, Scale,
};

use super::*;

#[test]
fn compiling_a_grid_produces_a_component_named_after_it() {
    let source_id = Uuid::new_v4();
    let mut grid = LogicGrid::new();
    for (index, kind) in [
        ComponentKind::Input {
            scale: Scale::ONE,
            id: InputId::from_u128(1),
            label: "IN".to_owned(),
        },
        ComponentKind::Output {
            scale: Scale::ONE,
            id: OutputId::from_u128(1),
            label: "OUT".to_owned(),
        },
        ComponentKind::Not { scale: Scale::ONE },
    ]
    .into_iter()
    .enumerate()
    {
        let component = Component {
            id: grid.next_component_id(),
            position: Point::new(0, index as i64 * 4),
            orientation: ComponentOrientation::Up,
            kind,
        };
        LogicGrid::apply_operation(&mut grid, &LogicGridOperation::AddComponent { component });
    }

    let compiled = generate_initial(source_id, &grid).unwrap();

    assert_eq!(compiled.source(), source_id);
    assert_eq!(compiled.ports().len(), 2);
    assert_eq!(artifact_name("Half Adder"), "Half Adder Component");
}
