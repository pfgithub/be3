use super::*;

#[test]
fn child_placements_round_trip() {
    let message = Message::Children(ChildPlacements {
        instance: EditorInstanceId(4),
        region: EditorRegion::Frame,
        generation: 9,
        children: vec![
            ChildPlacement {
                child: ChildId(1),
                block_id: [3; 16],
                block_type: [5; 16],
                rect: ChildRect {
                    x: 12.0,
                    y: 24.0,
                    width: 320.0,
                    height: 180.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 480.0,
                },
                corner_radius: 6.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Passive,
                intrinsic_width: 0.0,
                intrinsic_height: 0.0,
                rotation: 0.0,
                opacity: 1.0,
            },
            ChildPlacement {
                child: ChildId(2),
                block_id: [7; 16],
                block_type: [5; 16],
                rect: ChildRect {
                    x: 12.0,
                    y: 220.0,
                    width: 320.0,
                    height: 180.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 480.0,
                },
                corner_radius: 0.0,
                layer: ChildLayer::Above,
                mode: ChildMode::Live,
                intrinsic_width: 0.0,
                intrinsic_height: 0.0,
                rotation: 0.0,
                opacity: 1.0,
            },
            ChildPlacement {
                child: ChildId(3),
                block_id: [7; 16],
                block_type: [5; 16],
                rect: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 32.0,
                },
                clip: ChildRect {
                    x: 0.0,
                    y: 0.0,
                    width: 640.0,
                    height: 480.0,
                },
                corner_radius: 0.0,
                layer: ChildLayer::Below,
                mode: ChildMode::Live,
                intrinsic_width: 0.0,
                intrinsic_height: 0.0,
                rotation: 0.0,
                opacity: 1.0,
            },
        ],
        occluders: vec![Occluder {
            after: 1,
            rect: ChildRect {
                x: 0.0,
                y: 0.0,
                width: 64.0,
                height: 480.0,
            },
        }],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let Message::Children(placements) = &message else {
        panic!("the message is a child placement table");
    };
    assert!(placements.occluded(0, 8.0, 8.0));
    assert!(!placements.occluded(1, 8.0, 8.0));
}
