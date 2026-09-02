use super::*;

fn placements(occluders: Vec<Occluder>) -> Message {
    Message::Children(ChildPlacements {
        instance: EditorInstanceId(1),
        region: EditorRegion::Frame,
        generation: 1,
        children: vec![ChildPlacement {
            child: ChildId(1),
            block_id: [1; 16],
            block_type: [2; 16],
            rect: ChildRect::default(),
            clip: ChildRect::default(),
            corner_radius: 0.0,
            layer: ChildLayer::Below,
            mode: ChildMode::Preview,
            intrinsic_width: 0.0,
            intrinsic_height: 0.0,
            rotation: 0.0,
            opacity: 1.0,
        }],
        occluders,
    })
}

#[test]
fn rejects_unordered_occluders() {
    let occluder = |after| Occluder {
        after,
        rect: ChildRect::default(),
    };
    assert_eq!(
        encode_frame(&placements(vec![occluder(2)])),
        Err(DecodeError::MalformedPayload)
    );
    assert_eq!(
        encode_frame(&placements(vec![occluder(1), occluder(0)])),
        Err(DecodeError::MalformedPayload)
    );
    assert!(encode_frame(&placements(vec![occluder(0), occluder(1)])).is_ok());
}
