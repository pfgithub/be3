mod support;

use block::{BlockParent, BlockReferenceList, ServerMessage};
use support::{create, references, set_parent, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn lists_backrefs_with_relationship_metadata() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let target = Uuid::new_v4();
    let source = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, target, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut socket, source, vec![target]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, source, BlockParent::Root).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, target, BlockParent::Uuid(source)).await,
        ServerMessage::Ok { .. }
    ));

    let blocks = references(&mut socket, BlockReferenceList::Backrefs(target)).await;
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].id, source);
    assert_eq!(blocks[0].parent, BlockParent::Root);
    assert_eq!(blocks[0].references, 1);

    server.cleanup().await;
}
