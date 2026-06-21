use super::*;

#[test]
fn led_rotation_moves_its_input_with_its_short_edge() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(10, 20),
        rotation: Rotation::Right,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Led,
    };

    assert_eq!(component.size(), Some(Size::new(2, 1)));
    assert_eq!(
        component.connection_slots(),
        vec![ConnectionSlot {
            id: ConnectionSlotId(0),
            direction: ConnectionDirection::Input,
            side: ComponentSide::Left,
            start: 20,
            end: 21,
            scale: Scale::ONE,
        }]
    );
}
