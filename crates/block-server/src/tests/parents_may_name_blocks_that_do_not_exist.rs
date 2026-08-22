use super::support::{create, parent as read_parent, read, references, set_parent, TestServer};
use block::{BlockParent, BlockReferenceList, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn parents_may_name_blocks_that_do_not_exist() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let child = Uuid::new_v4();
    let parent = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, child, vec![]).await,
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
    assert!(references(&mut socket, BlockReferenceList::Parents(child))
        .await
        .is_empty());

    assert!(matches!(
        create(&mut socket, parent, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    let listed = references(&mut socket, BlockReferenceList::Parents(child)).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, parent);
    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Uuid(parent)
    );
    server.cleanup().await;
}
