use super::support::{create, references, set_parent, TestServer};
use block::{BlockParent, BlockReferenceList, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn lists_every_root_block_in_uuid_order() {
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
        set_parent(&mut socket, parent, BlockParent::Root).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));

    assert_eq!(
        references(&mut socket, BlockReferenceList::Roots)
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![parent]
    );
    server.cleanup().await;
}
