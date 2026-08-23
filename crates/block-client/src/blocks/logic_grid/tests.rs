use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, ComponentPort, ComponentSide,
    Point, Scale, Size, Wire,
};
use uuid::Uuid;

use super::{LogicGrid, LogicGridOperation};
use crate::{BlockClient, BlockHandle};

fn client_with_grid() -> (BlockClient, BlockHandle<LogicGrid>) {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.create_block(LogicGrid::new());
    (client, block)
}

fn led(id: ComponentId, position: Point) -> Component {
    Component {
        id,
        position,
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::Led,
    }
}

fn subcomponent(id: ComponentId, compiled: Uuid) -> Component {
    Component {
        id,
        position: Point::new(0, 0),
        orientation: ComponentOrientation::Up,
        kind: ComponentKind::subcomponent(
            compiled,
            Size::new(2, 2),
            vec![ComponentPort::input(
                0,
                Scale::ONE,
                ComponentSide::Left,
                0,
                1,
            )],
        )
        .unwrap(),
    }
}

fn wire(start: (i64, i64), end: (i64, i64)) -> Wire {
    Wire::new(
        Point::new(start.0, start.1),
        Point::new(end.0, end.1),
        Scale::ONE,
    )
    .unwrap()
}

                                                                               
                                                     
fn add(block: &BlockHandle<LogicGrid>, make: impl FnOnce(ComponentId) -> Component) -> ComponentId {
    let id = block.read().unwrap().next_component_id();
    block.operate(LogicGridOperation::AddComponent {
        component: make(id),
    });
    id
}

mod logic_grid_history_restores_removed_components;
mod logic_grid_history_restores_wires_split_by_a_removal;
mod logic_grid_references_called_blocks;
mod logic_grid_repeated_additions_are_ignored;
mod logic_grid_serialization_round_trips;
