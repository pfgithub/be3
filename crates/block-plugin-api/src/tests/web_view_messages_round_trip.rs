use super::*;

#[test]
fn web_view_messages_round_trip() {
    for message in [
        Message::Editor(EditorMessage::WebView {
            instance: EditorInstanceId(5),
            region: EditorRegion::Frame,
            rect: Some(ChildRect {
                x: 4.0,
                y: 8.0,
                width: 320.0,
                height: 240.0,
            }),
        }),
        Message::Editor(EditorMessage::WebView {
            instance: EditorInstanceId(5),
            region: EditorRegion::Frame,
            rect: None,
        }),
        Message::Editor(EditorMessage::WebViewCommand {
            instance: EditorInstanceId(5),
            command: WebViewCommand::Open("https://example.com/".into()),
        }),
        Message::Editor(EditorMessage::WebViewCommand {
            instance: EditorInstanceId(5),
            command: WebViewCommand::Reload,
        }),
        Message::Editor(EditorMessage::WebViewEvent {
            instance: EditorInstanceId(5),
            event: WebViewEvent::History(-1),
        }),
        Message::Editor(EditorMessage::WebViewEvent {
            instance: EditorInstanceId(5),
            event: WebViewEvent::Title("Example Domain".into()),
        }),
    ] {
        assert_eq!(
            decode_frame(&encode_frame(&message).unwrap()).unwrap(),
            message
        );
    }
}
