use super::support::{references, request, TestServer};
use block::{BlockReferenceList, ClientMessage, ReferenceDelta, ServerMessage};
use uuid::Uuid;

/// The server never reads what a block holds, so a block reports for itself
/// whether it was generated from another one. Listings hand that back so a
/// client can mark generated blocks without opening them.
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
                implicit_name: "Generated".into(),
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

    // Unlinking the block from its source is an ordinary update.
    assert!(matches!(
        request(
            &mut socket,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id,
                seq: None,
                operation_id: Uuid::new_v4(),
                operation: vec![],
                implicit_name: "Generated".into(),
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
