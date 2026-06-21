use super::*;

#[test]
fn not_gate_defines_rotated_input_and_output_slots() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        orientation: ComponentOrientation::Right,
        kind: ComponentKind::Not { scale: scale(2) },
    };

    assert_eq!(
        component.connection_slots(),
        vec![
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Left,
                start: 20,
                end: 22,
                scale: scale(2),
            },
            ConnectionSlot {
                id: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 20,
                end: 22,
                scale: scale(2),
            },
        ]
    );
}
