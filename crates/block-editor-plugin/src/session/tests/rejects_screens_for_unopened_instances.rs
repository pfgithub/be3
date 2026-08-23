use super::*;

#[test]
fn rejects_screens_for_unopened_instances() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let responses = session.receive(Message::Screens(ScreenSet {
        request_id: 1,
        screens: vec![screen(ScreenId(1), EditorInstanceId(9))],
    }));
    assert!(matches!(responses.as_slice(), [Message::Error(_)]));
    assert_eq!(session.state(), State::Failed);
}
