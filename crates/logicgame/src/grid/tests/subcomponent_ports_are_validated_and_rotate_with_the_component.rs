use super::*;

#[test]
fn subcomponent_ports_are_validated_and_rotate_with_the_component() {
    let size = Size::new(7, 11);
    let ports = vec![
        ComponentPort::input(0, scale(2), ComponentSide::Left, 2, 5),
        ComponentPort::output(0, scale(4), ComponentSide::Bottom, 3, 7),
    ];
    let kind = ComponentKind::subcomponent(file_id(), component_hash(), size, ports).unwrap();
    let mut grid = LogicGrid::new();
    let id = grid.add_component(Point::new(8, 4), Rotation::Left, kind);
    let component = grid.component(id).unwrap();
    assert_eq!(component.size(), Some(Size::new(11, 7)));
    assert_eq!(
        component.connection_slots(),
        vec![
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Bottom,
                start: 10,
                end: 13,
                scale: scale(2),
            },
            ConnectionSlot {
                id: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 4,
                end: 8,
                scale: scale(4),
            },
        ]
    );
    assert!(grid.validate().is_empty());

    let invalid = ComponentPort::input(0, scale(2), ComponentSide::Top, 6, 8);
    assert_eq!(
        ComponentKind::subcomponent(file_id(), component_hash(), size, vec![invalid]),
        Err(GeometryError::InvalidSubcomponentPort {
            size,
            port: invalid,
        })
    );
}
