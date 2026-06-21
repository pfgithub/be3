use super::*;

#[test]
fn input_components_rotate_their_connection_slots() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Right,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Input {
            scale: scale(2),
            id: InputId::from_u128(0),

            label: String::new(),
        },
    };

    assert_eq!(component.size(), Some(Size::new(2, 2)));
    assert_eq!(
        component.connection_slots(),
        vec![ConnectionSlot {
            id: ConnectionSlotId(0),
            direction: ConnectionDirection::Output,
            side: ComponentSide::Left,
            start: 20,
            end: 22,
            scale: scale(2),
        }]
    );
    assert_eq!(
        component.lead(),
        Some(ComponentLead {
            side: ComponentSide::Right,
            axis: 12,
            start: 20,
            end: 22,
        })
    );
}
