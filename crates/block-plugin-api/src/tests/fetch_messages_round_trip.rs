use super::*;

#[test]
fn fetch_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::Fetch {
            instance: EditorInstanceId(2),
            request_id: 9,
            url: "https://api.github.com/repos/pfgithub/be3/git/trees/dev:snapshots".into(),
        }),
        Message::Editor(EditorMessage::Fetched {
            instance: EditorInstanceId(2),
            request_id: 9,
            result: FetchResult::Body(vec![4; MAX_STRING_BYTES + 1]),
        }),
        Message::Editor(EditorMessage::Fetched {
            instance: EditorInstanceId(2),
            request_id: 10,
            result: FetchResult::Failed("api.github.com answered 404".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
