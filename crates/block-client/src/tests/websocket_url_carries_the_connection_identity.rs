use super::*;

/// The identity travels in the query string rather than in headers because a
/// browser cannot set headers on a websocket handshake.
#[test]
fn websocket_url_carries_the_connection_identity() {
    let workspace_id = Uuid::parse_str("7c3e8b2d-1a5f-4c7b-9d2e-3f4a5b6c7d8e").unwrap();
    assert_eq!(
        websocket_url("https://blocks.example.com", "the-token", workspace_id),
        "wss://blocks.example.com/api?token=the-token\
         &workspace=7c3e8b2d-1a5f-4c7b-9d2e-3f4a5b6c7d8e"
    );
}
