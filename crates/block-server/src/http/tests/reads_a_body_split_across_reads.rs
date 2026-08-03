use super::*;

#[tokio::test]
async fn reads_a_body_split_across_reads() {
    // The first read stops part-way through the body, so the rest has to be
    // read after the head has already been parsed.
    let head: &[u8] = b"POST /management HTTP/1.1\r\nContent-Length: 9\r\n\r\n{\"a\":";
    let rest: &[u8] = b"\"b\"}";
    let mut stream = head.chain(rest);
    let request = read_head(&mut stream).await.unwrap();
    assert_eq!(request.head.method, "POST");
    assert_eq!(request.head.path, "/management");
    assert!(!request.head.is_websocket_upgrade());
    assert_eq!(
        read_body(&mut stream, &request).await.unwrap(),
        br#"{"a":"b"}"#
    );
}
