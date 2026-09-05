use super::*;

#[test]
fn block_types_round_trip() {
    let message = Message::BlockTypes(vec![BlockTypeDescriptor {
        block_type: [7; 16],
        display_name: "Folder".into(),
        icon_codepoint: "\u{e2c7}".into(),
        children: ChildOperations {
            add: true,
            delete: true,
            replace: false,
        },
    }]);
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let oversized = Message::BlockTypes(vec![BlockTypeDescriptor {
        block_type: [7; 16],
        display_name: "x".repeat(MAX_STRING_BYTES + 1),
        icon_codepoint: String::new(),
        children: ChildOperations::default(),
    }]);
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("string"))
    );
}
