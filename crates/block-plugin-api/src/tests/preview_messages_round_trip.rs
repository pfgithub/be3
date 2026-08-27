use super::*;

#[test]
fn preview_messages_round_trip() {
    let message = Message::Previews(PreviewLayout {
        generation: 7,
        width: 512,
        height: 256,
        scale_factor_millis: 2000,
        slots: vec![
            PreviewSlot {
                instance: EditorInstanceId(1),
                region: EditorRegion::LeftSidebar,
                child: ChildId(3),
                x: 0,
                y: 0,
                width: 352,
                height: 208,
            },
            PreviewSlot {
                instance: EditorInstanceId(1),
                region: EditorRegion::Preview,
                child: ChildId(4),
                x: 352,
                y: 0,
                width: 160,
                height: 96,
            },
        ],
    });
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let Message::Previews(layout) = &message else {
        panic!("the message is a preview layout");
    };
    assert_eq!(layout.scale_factor(), 2.0);
    assert_eq!(
        layout
            .slot(EditorInstanceId(1), EditorRegion::Preview, ChildId(4))
            .map(|slot| slot.x),
        Some(352)
    );
    assert!(layout
        .slot(EditorInstanceId(2), EditorRegion::Preview, ChildId(4))
        .is_none());

    let ready = Message::PreviewsReady { generation: 7 };
    assert_eq!(decode_frame(&encode_frame(&ready).unwrap()).unwrap(), ready);
}
