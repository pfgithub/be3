use super::*;

#[test]
fn merger_combines_smaller_inputs_into_a_wide_output() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        rotation: Rotation::Right,
        kind: ComponentKind::MergerSplitter {
            input_scale: scale(4),
            output_scale: scale(16),
        },
    };
    let slots = component.connection_slots();

    assert_eq!(slots.len(), 5);
    assert!(slots[..4].iter().all(|slot| {
        slot.direction == ConnectionDirection::Input && slot.side == ComponentSide::Left
    }));
    assert_eq!(
        slots[4],
        ConnectionSlot {
            id: ConnectionSlotId(4),
            direction: ConnectionDirection::Output,
            side: ComponentSide::Right,
            start: 0,
            end: 16,
            scale: scale(16),
        }
    );
}
