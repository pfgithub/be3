mod support;

use block::{BlockParent, BlockReferenceList, ServerMessage};
use support::{create, parent as read_parent, read, references, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn reparents_without_changing_either_parents_references() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let first_parent = Uuid::new_v4();
    let second_parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    for (id, references) in [
        (child, vec![]),
        (first_parent, vec![child]),
        (second_parent, vec![child]),
    ] {
        assert!(matches!(
            create(&mut socket, id, references).await,
            ServerMessage::Ok { .. }
        ));
    }
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(first_parent)).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(second_parent)).await,
        ServerMessage::Ok { .. }
    ));

    let mut expected_backrefs = vec![first_parent, second_parent];
    expected_backrefs.sort_unstable();
    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Uuid(second_parent)
    );
    assert_eq!(
        references(&mut socket, BlockReferenceList::References(first_parent))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![child]
    );
    assert_eq!(
        references(&mut socket, BlockReferenceList::References(second_parent))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![child]
    );
    assert_eq!(
        references(&mut socket, BlockReferenceList::Backrefs(child))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        expected_backrefs
    );
    server.cleanup().await;
}
