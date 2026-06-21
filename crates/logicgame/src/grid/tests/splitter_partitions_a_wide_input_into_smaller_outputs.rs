use super::*;

#[test]
fn splitter_partitions_a_wide_input_into_smaller_outputs() {
    let component = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        rotation: Rotation::Right,
        flip: ComponentFlip::default(),
        kind: ComponentKind::MergerSplitter {
            input_scale: scale(16),
            output_scale: scale(4),
        },
    };

    assert_eq!(component.size(), Some(Size::new(16, 16)));
    assert_eq!(
        component.connection_slots(),
        vec![
            ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Left,
                start: 0,
                end: 16,
                scale: scale(16),
            },
            ConnectionSlot {
                id: ConnectionSlotId(1),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 0,
                end: 4,
                scale: scale(4),
            },
            ConnectionSlot {
                id: ConnectionSlotId(2),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 4,
                end: 8,
                scale: scale(4),
            },
            ConnectionSlot {
                id: ConnectionSlotId(3),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 8,
                end: 12,
                scale: scale(4),
            },
            ConnectionSlot {
                id: ConnectionSlotId(4),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Right,
                start: 12,
                end: 16,
                scale: scale(4),
            },
        ]
    );
}
