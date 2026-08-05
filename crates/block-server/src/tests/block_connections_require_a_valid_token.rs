use super::support::TestServer;

#[tokio::test]
async fn block_connections_require_a_valid_token() {
    let server = TestServer::start().await;

    // A garbage token and a missing one are both refused before the
    // websocket handshake completes; only a real session token works.
    assert!(server
        .try_connect_to("not-a-real-token", server.workspace_id)
        .await
        .is_err());
    assert!(server
        .try_connect_to("", server.workspace_id)
        .await
        .is_err());
    assert!(server
        .try_connect_to(&server.token, server.workspace_id)
        .await
        .is_ok());
    server.cleanup().await;
}
