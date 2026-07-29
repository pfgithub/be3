mod support;

use block::ServerMessage;
use support::{create, orphaned, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn lists_every_block_without_a_parent_in_uuid_order() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, child, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut socket, parent, vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, Some(parent)).await,
        ServerMessage::Ok { .. }
    ));

    assert_eq!(orphaned(&mut socket).await, vec![parent]);
    server.cleanup().await;
}
