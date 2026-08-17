use super::*;

#[test]
fn repeated_start_and_shutdown_are_clean() {
    let mut session = running_session();
    session.shutdown(10);
    session.shutdown(11);
    assert_eq!(session.next_outbound(), Some(Message::Shutdown));
    assert_eq!(session.next_outbound(), None);
    session.receive(Message::ShutdownAcknowledged, 12);
    assert_eq!(session.state(), &SessionState::Closed);
    session.start(20);
    session.receive(hello(), 21);
    assert_eq!(session.state(), &SessionState::Running);
}
