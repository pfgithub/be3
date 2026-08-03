use super::*;

#[test]
fn websocket_url_matches_the_server_scheme() {
    assert_eq!(
        websocket_url("http://127.0.0.1:8080"),
        "ws://127.0.0.1:8080"
    );
    assert_eq!(
        websocket_url("https://blocks.example.com"),
        "wss://blocks.example.com"
    );
}
