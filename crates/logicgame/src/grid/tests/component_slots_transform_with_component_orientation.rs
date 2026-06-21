use super::*;

#[test]
fn component_slots_transform_with_component_orientation() {
    let size = Size::new(10, 6);
    let ports = vec![
        ComponentPort::input(0, scale(1), ComponentSide::Left, 1, 3),
        ComponentPort::output(0, scale(1), ComponentSide::Top, 2, 5),
    ];
    let kind = ComponentKind::subcomponent(file_id(), component_hash(), size, ports).unwrap();
    let mut grid = LogicGrid::new();
    let id = grid.add_component(Point::new(4, 7), Rotation::Up, kind);

    assert!(grid.set_component_orientation(id, ComponentOrientation::Down));

    assert_eq!(
        grid.component(id).unwrap().connection_slots(),
        vec![
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Right,
                start: 10,
                end: 12,
                scale: scale(1),
            },
            ConnectionSlot {
                id: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Bottom,
                start: 9,
                end: 12,
                scale: scale(1),
            },
        ]
    );
}
