use std::collections::BTreeMap;

use super::support::{create, create_workspace, references, request, TestServer};
use block::{BlockReferenceList, ClientMessage, ReferenceDelta, ServerMessage};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn workspaces_isolate_identical_block_ids_and_notifications() {
    let server = TestServer::start().await;
    let management = server.management();
    let second = create_workspace(&management, &server.token, "Second").await;
    let mut first_socket = server.connect().await;
    let mut second_socket = server.connect_to(&server.token, second.id).await;
    let shared_id = Uuid::new_v4();
    assert!(matches!(
        create(&mut first_socket, shared_id, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut second_socket, shared_id, vec![]).await,
        ServerMessage::Ok { .. }
    ));

    let first_roots = references(&mut first_socket, BlockReferenceList::Orphans).await;
    let second_roots = references(&mut second_socket, BlockReferenceList::Orphans).await;
    assert_eq!(first_roots.len(), 1);
    assert_eq!(second_roots.len(), 1);

    let second_only = Uuid::new_v4();
    create(&mut second_socket, second_only, vec![]).await;
    let referrer = Uuid::new_v4();
    assert!(matches!(
        create(&mut first_socket, referrer, vec![second_only]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(
        references(&mut first_socket, BlockReferenceList::References(referrer))
            .await
            .is_empty()
    );
    assert!(references(
        &mut second_socket,
        BlockReferenceList::Backrefs(second_only)
    )
    .await
    .is_empty());

    let _ = request(
        &mut first_socket,
        ClientMessage::ReadBlock {
            request_id: Uuid::new_v4(),
            id: shared_id,
            watch: true,
        },
    )
    .await;
    let response = request(
        &mut second_socket,
        ClientMessage::UpdateBlock {
            request_id: Uuid::new_v4(),
            id: shared_id,
            seq: None,
            operation_id: Uuid::new_v4(),
            operation: vec![1],
            properties: BTreeMap::new(),
            dynamic_artifact: false,
            references: ReferenceDelta::default(),
        },
    )
    .await;
    assert!(matches!(response, ServerMessage::Ok { .. }));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), first_socket.next())
            .await
            .is_err()
    );

    server.cleanup().await;
}
