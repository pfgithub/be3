mod support;

use block::{BlockParent, BlockReferenceList, ServerMessage};
use support::{create, parent as read_parent, read, references, set_parent, update, TestServer};
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
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        update(&mut socket, parent, vec![], vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(references(&mut socket, BlockReferenceList::Orphans)
        .await
        .iter()
        .any(|block| block.id == child));

    assert!(matches!(
        update(&mut socket, parent, vec![child], vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Orphaned
    );
    server.cleanup().await;
}
