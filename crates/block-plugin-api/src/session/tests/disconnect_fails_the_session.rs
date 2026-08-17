use super::*;

#[test]
fn disconnect_fails_the_session() {
    let mut session = running_session();
    session.disconnected();
    assert_eq!(
        session.state(),
        &SessionState::Failed(SessionFailure::Disconnected)
    );
}
