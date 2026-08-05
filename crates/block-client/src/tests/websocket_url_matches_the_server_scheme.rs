use super::*;

#[test]
fn websocket_url_matches_the_server_scheme() {
    let workspace_id = Uuid::nil();
    assert!(
        websocket_url("http://127.0.0.1:8080", "token", workspace_id)
            .starts_with("ws://127.0.0.1:8080/")
    );
    assert!(
        websocket_url("https://blocks.example.com", "token", workspace_id)
            .starts_with("wss://blocks.example.com/")
    );
}
