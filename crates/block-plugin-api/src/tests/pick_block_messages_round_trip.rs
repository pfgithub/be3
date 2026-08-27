use super::*;

#[test]
fn pick_block_messages_round_trip() {
    let request = Message::Editor(EditorMessage::PickBlock {
        instance: EditorInstanceId(2),
        request_id: 7,
        filter: BlockFilter {
            name: "Slide".into(),
            block_types: vec![[9; 16]],
            templates: true,
        },
    });
    assert_eq!(
        decode_frame(&encode_frame(&request).unwrap()).unwrap(),
        request
    );

    for pick in [
        BlockPick::Chosen {
            block_id: [1; 16],
            block_type: [9; 16],
        },
        BlockPick::Cancelled,
        BlockPick::Failed("the block could not be created".into()),
    ] {
        let message = Message::Editor(EditorMessage::BlockPicked {
            instance: EditorInstanceId(2),
            request_id: 7,
            pick,
        });
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }

    let oversized = Message::Editor(EditorMessage::PickBlock {
        instance: EditorInstanceId(2),
        request_id: 8,
        filter: BlockFilter {
            name: "x".repeat(MAX_STRING_BYTES + 1),
            block_types: Vec::new(),
            templates: false,
        },
    });
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
