use super::*;

#[test]
fn storage_has_bottom_input_and_top_output() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Up,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Storage {
            scale: scale(2),
            value: 0,
        },
    };

    assert_eq!(
        component.connection_slots(),
        vec![
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Bottom,
                start: 10,
                end: 12,
                scale: scale(2),
            },
            ConnectionSlot {
                id: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Top,
                start: 10,
                end: 12,
                scale: scale(2),
            },
        ]
    );
}
