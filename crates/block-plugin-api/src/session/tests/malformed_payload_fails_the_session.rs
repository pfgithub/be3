use super::*;

#[test]
fn malformed_payload_fails_the_session() {
    let mut session = session();
    session.start(0);
    session.receive_frame(&[0, 0, 0, 1, 255], 1);
    assert_eq!(
        session.state(),
        &SessionState::Failed(SessionFailure::MalformedMessage)
    );
}
