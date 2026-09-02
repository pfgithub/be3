use super::*;

#[test]
fn child_statuses_round_trip() {
    let message = Message::ChildStatuses(vec![
        ChildStatus {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
            child: ChildId(1),
            available: true,
            intrinsic_width: 320.0,
            intrinsic_height: 180.0,
            aspect_ratio: 16.0 / 9.0,
            hovered: true,
            active: false,
            interaction: InteractionMode::Live,
            error: None,
        },
        ChildStatus {
            instance: EditorInstanceId(4),
            region: EditorRegion::Frame,
            child: ChildId(2),
            available: false,
            intrinsic_width: 0.0,
            intrinsic_height: 0.0,
            aspect_ratio: 0.0,
            hovered: false,
            active: false,
            interaction: InteractionMode::Live,
            error: Some("the block is already open above this editor".into()),
        },
    ]);
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let oversized = Message::ChildStatuses(vec![ChildStatus {
        instance: EditorInstanceId(4),
        region: EditorRegion::Frame,
        child: ChildId(3),
        available: false,
        intrinsic_width: 0.0,
        intrinsic_height: 0.0,
        aspect_ratio: 0.0,
        hovered: false,
        active: false,
        interaction: InteractionMode::Live,
        error: Some("x".repeat(MAX_STRING_BYTES + 1)),
    }]);
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
