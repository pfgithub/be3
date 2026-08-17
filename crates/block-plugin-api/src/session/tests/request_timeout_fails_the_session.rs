use super::*;

#[test]
fn request_timeout_fails_the_session() {
    let mut session = running_session();
    session
        .enqueue_request(42, Message::Ping { nonce: 1 }, 10)
        .unwrap();
    session.tick(10 + REQUEST_TIMEOUT_MILLISECONDS);
    assert_eq!(
        session.state(),
        &SessionState::Failed(SessionFailure::RequestTimedOut(42))
    );
}
