mod support;

use block::{ErrorCode, ServerMessage};
use support::{create, read, update, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn missing_references_reject_creates_and_do_not_commit_updates() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let block = Uuid::new_v4();
    let missing = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, block, vec![missing]).await,
        ServerMessage::Error {
            code: ErrorCode::ReferencedBlockNotFound,
            ..
        }
    ));
    assert!(matches!(
        create(&mut socket, block, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        update(&mut socket, block, vec![missing], vec![]).await,
        ServerMessage::Error {
            code: ErrorCode::ReferencedBlockNotFound,
            ..
        }
    ));

    let ServerMessage::ReadBlock { operations, .. } = read(&mut socket, block).await else {
        panic!("expected block read");
    };
    assert!(operations.is_empty());
    server.cleanup().await;
}
