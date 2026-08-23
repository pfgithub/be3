use super::*;

#[test]
fn a_superseded_request_is_forgotten() {
    let mut session = running_session();
    session.send(screens(1), 0).unwrap();
    session.send(screens(2), 0).unwrap();
    assert_eq!(session.queued_message_count(), 1);
    assert_eq!(session.pending_request_count(), 1);
    session.receive(Message::Acknowledged { request_id: 2 }, 1);
    assert_eq!(session.state(), &SessionState::Running);
    session.tick(REQUEST_TIMEOUT_MILLISECONDS * 2);
    assert_eq!(session.state(), &SessionState::Running);
}
