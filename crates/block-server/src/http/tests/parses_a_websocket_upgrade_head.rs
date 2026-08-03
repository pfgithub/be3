use super::*;

#[tokio::test]
async fn parses_a_websocket_upgrade_head() {
    let mut stream: &[u8] = b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: WebSocket\r\nX-Block-Account-Id: 42\r\n\r\n";
    let request = read_head(&mut stream).await.unwrap();
    assert!(request.head.is_websocket_upgrade());
    assert_eq!(request.head.header("x-block-account-id"), Some("42"));
    // The handshake is replayed to the websocket implementation, so every byte
    // has to survive the peek.
    assert_eq!(request.buffered.len(), request.head_len);
    assert!(request.buffered.starts_with(b"GET / HTTP/1.1"));
}
