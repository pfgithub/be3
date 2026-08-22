use std::collections::BTreeMap;

use super::support::{request, watch, TestServer};
use block::{BlockAccess, BlockParent, ClientMessage, ErrorCode, ServerMessage};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn watching_a_block_before_it_exists_delivers_it_on_creation() {
    let server = TestServer::start().await;
    let mut watcher = server.connect().await;
    let mut writer = server.connect().await;
    let id = Uuid::new_v4();
    let block_type = Uuid::new_v4();

    assert!(matches!(
        watch(&mut watcher, id).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));
    assert!(matches!(
        request(
            &mut writer,
            ClientMessage::CreateBlock {
                request_id: Uuid::new_v4(),
                id,
                block_type,
                data: vec![7],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references: vec![],
                watch: false,
            },
        )
        .await,
        ServerMessage::Ok { .. }
    ));

    let message = watcher.next().await.unwrap().unwrap();
    let message: ServerMessage = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    let ServerMessage::BlockCreated {
        id: created,
        block_type: created_type,
        author,
        snapshot,
        snapshot_seq,
        parent,
        access,
        ..
    } = message
    else {
        panic!("expected the created block, got {message:?}");
    };
    assert_eq!(created, id);
    assert_eq!(created_type, block_type);
    assert_eq!(author, server.account_id);
    assert_eq!(snapshot, vec![7]);
    assert_eq!(snapshot_seq, 0);
    assert_eq!(parent, BlockParent::Orphaned);
    assert_eq!(access, BlockAccess::Edit);
    server.cleanup().await;
}
