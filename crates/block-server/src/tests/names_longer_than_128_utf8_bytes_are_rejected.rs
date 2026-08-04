use super::support::{request, TestServer};
use block::{ClientMessage, ErrorCode, ServerMessage, MAX_NAME_BYTES};
use uuid::Uuid;

#[tokio::test]
async fn names_longer_than_128_utf8_bytes_are_rejected() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let response = request(
        &mut socket,
        ClientMessage::CreateBlock {
            request_id: Uuid::new_v4(),
            id: Uuid::new_v4(),
            block_type: Uuid::new_v4(),
            data: vec![],
            implicit_name: "a".repeat(MAX_NAME_BYTES + 1),
            dynamic_artifact: false,
            references: vec![],
            watch: false,
        },
    )
    .await;

    assert!(matches!(
        response,
        ServerMessage::Error {
            code: ErrorCode::InvalidMessage,
            ..
        }
    ));
    server.cleanup().await;
}
