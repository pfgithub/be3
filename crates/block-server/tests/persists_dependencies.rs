mod support;

use block::{BlockParent, ServerMessage};
use support::{create, read, relationships, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn dependency_state_survives_a_server_restart() {
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
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));
    drop(socket);
    let root = server.stop().await;

    let restarted = TestServer::start_at(root).await;
    let mut socket = restarted.connect().await;
    assert_eq!(
        relationships(read(&mut socket, child).await),
        (BlockParent::Uuid(parent), vec![], vec![parent])
    );
    restarted.cleanup().await;
}
