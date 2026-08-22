use super::support::{create, parent as read_parent, read, set_parent, update, TestServer};
use block::{BlockParent, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn a_parent_need_not_reference_its_child() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, child, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut socket, parent, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));

    assert!(matches!(
        update(&mut socket, parent, vec![child], vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        update(&mut socket, parent, vec![], vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Uuid(parent)
    );
    server.cleanup().await;
}
