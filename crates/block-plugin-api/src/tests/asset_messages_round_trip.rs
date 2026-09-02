use super::*;

#[test]
fn asset_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::ReadAsset {
            instance: EditorInstanceId(3),
            request_id: 1,
            name: "games.json".into(),
        }),
        Message::Editor(EditorMessage::AssetRead {
            instance: EditorInstanceId(3),
            request_id: 1,
            result: AssetResult::Body(vec![7; MAX_STRING_BYTES + 1]),
        }),
        Message::Editor(EditorMessage::AssetRead {
            instance: EditorInstanceId(3),
            request_id: 2,
            result: AssetResult::Failed("connect_four.wasm is not beside the app".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
