use super::*;

#[test]
fn accepts_previews_ready() {
    let mut session = ClientSession::new("demo", "Demo", "1.0");
    accept(&mut session);
    let instance = EditorInstanceId(1);
    open(&mut session, instance);
    assert_eq!(session.state(), State::Running);

    let answers = session.receive(Message::PreviewsReady { generation: 3 });

    assert!(answers.is_empty(), "a drawn preview needs no answer");
    assert_eq!(session.state(), State::Running);
}
