mod support;

use block::ServerMessage;
use support::{create, orphaned, read, relationships, set_parent, update, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn removing_a_parent_reference_orphans_the_child_without_restoring_it_on_readd() {
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
    assert!(matches!(
        update(&mut socket, parent, vec![], vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(orphaned(&mut socket).await.contains(&child));

    assert!(matches!(
        update(&mut socket, parent, vec![child], vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert_eq!(relationships(read(&mut socket, child).await).0, None);
    server.cleanup().await;
}
