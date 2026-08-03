use super::*;

#[tokio::test]
async fn rejects_a_malformed_head() {
    let mut stream: &[u8] = b"not an HTTP request\r\n\r\n";
    let error = read_head(&mut stream).await.unwrap_err();
    assert!(
        matches!(error, ServerError::InvalidRequest(_)),
        "expected an invalid request error, got {error}"
    );

    // A head that ends early is a closed connection rather than a parse failure.
    let mut stream: &[u8] = b"GET / HTTP/1.1\r\nHost: localhost\r\n";
    let error = read_head(&mut stream).await.unwrap_err();
    assert!(
        matches!(error, ServerError::InvalidRequest(_)),
        "expected an invalid request error, got {error}"
    );
}
