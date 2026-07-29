mod support;

use block::{BlockParent, ServerMessage};
use support::{create, read, relationships, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn read_returns_parent_references_and_all_backrefs() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let parent = Uuid::new_v4();
    let other = Uuid::new_v4();
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
        create(&mut socket, other, vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));

    let mut expected_backrefs = vec![parent, other];
    expected_backrefs.sort_unstable();
    assert_eq!(
        relationships(read(&mut socket, child).await),
        (BlockParent::Uuid(parent), vec![], expected_backrefs)
    );
    server.cleanup().await;
}
