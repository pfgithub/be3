use std::collections::BTreeMap;

use super::support::{references, request, TestServer};
use block::{BlockReferenceList, ClientMessage, ReferenceDelta, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn listings_report_whether_a_block_is_a_dynamic_artifact() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let id = Uuid::new_v4();

    assert!(matches!(
        request(
            &mut socket,
            ClientMessage::CreateBlock {
                request_id: Uuid::new_v4(),
                id,
                block_type: Uuid::new_v4(),
                data: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: true,
                references: vec![],
                watch: false,
            },
        )
        .await,
        ServerMessage::Ok { .. }
    ));
    let listed = references(&mut socket, BlockReferenceList::Orphans).await;
    assert_eq!(listed.len(), 1);
    assert!(listed[0].dynamic_artifact);

    assert!(matches!(
        request(
            &mut socket,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id,
                seq: None,
                operation_id: Uuid::new_v4(),
                operation: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references: ReferenceDelta::default(),
            },
        )
        .await,
        ServerMessage::Ok { .. }
    ));
    let listed = references(&mut socket, BlockReferenceList::Orphans).await;
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].dynamic_artifact);
}
