mod support;

use block::{BlockParent, ServerMessage};
use support::{create, parent as read_parent, read, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn read_returns_parent() {
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

    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Uuid(parent)
    );
    server.cleanup().await;
}
