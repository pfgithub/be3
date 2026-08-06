use std::collections::BTreeMap;

use super::support::{request, TestServer};
use block::{ClientMessage, ErrorCode, ServerMessage, MAX_PROPERTY_VALUE_BYTES};
use uuid::Uuid;

#[tokio::test]
async fn property_values_over_the_size_limit_are_rejected() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let mut properties = BTreeMap::new();
    properties.insert(Uuid::new_v4(), vec![0u8; MAX_PROPERTY_VALUE_BYTES + 1]);
    let response = request(
        &mut socket,
        ClientMessage::CreateBlock {
            request_id: Uuid::new_v4(),
            id: Uuid::new_v4(),
            block_type: Uuid::new_v4(),
            data: vec![],
            properties,
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
