mod support;

use block::{BlockParent, BlockReferenceList, ServerMessage};
use support::{create, references, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn lists_parents() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let root = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    for (id, references) in [(child, vec![]), (parent, vec![child]), (root, vec![parent])] {
        assert!(matches!(
            create(&mut socket, id, references).await,
            ServerMessage::Ok { .. }
        ));
    }
    assert!(matches!(
        set_parent(&mut socket, parent, BlockParent::Uuid(root)).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));

    assert_eq!(
        references(&mut socket, BlockReferenceList::Parents(child))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![root, parent]
    );

    server.cleanup().await;
}
