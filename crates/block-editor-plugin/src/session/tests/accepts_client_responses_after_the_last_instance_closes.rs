use super::*;

#[test]
fn accepts_client_responses_after_the_last_instance_closes() {
    let mut session = ClientSession::default();
    accept(&mut session);
    let instance = EditorInstanceId(1);
    open(&mut session, instance);
    session.receive(Message::Editor(block_plugin_api::EditorMessage::Close {
        instance,
    }));
    let responses = session.receive(Message::Client(TunnelMessage::Response {
        payload: "{\"status\":\"ok\"}".into(),
    }));
    assert!(responses.is_empty());
    assert_eq!(session.state(), State::Running);
}
