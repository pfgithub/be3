use super::*;

#[test]
fn accepts_ordered_lifecycle() {
    let mut session = ClientSession::default();
    assert!(matches!(session.hello(), Message::Hello(_)));
    accept(&mut session);
    assert_eq!(session.state(), State::Running);

    let instance = EditorInstanceId(1);
    open(&mut session, instance);
    let responses = session.receive(Message::Screens(ScreenSet {
        request_id: 7,
        screens: vec![screen(ScreenId(3), instance)],
    }));
    assert_eq!(responses, vec![Message::Acknowledged { request_id: 7 }]);
    assert!(session
        .receive(Message::Input(InputBatch {
            screen: ScreenId(3),
            events: Vec::new(),
        }))
        .is_empty());
    assert_eq!(
        session.receive(Message::Shutdown),
        vec![Message::ShutdownAcknowledged]
    );
    assert_eq!(session.state(), State::Closed);
}
