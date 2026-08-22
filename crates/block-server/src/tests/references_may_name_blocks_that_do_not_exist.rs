use super::support::{create, references, update, TestServer};
use block::{BlockReferenceList, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn references_may_name_blocks_that_do_not_exist() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let block = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, block, vec![first]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        update(&mut socket, block, vec![second], vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(
        references(&mut socket, BlockReferenceList::References(block))
            .await
            .is_empty()
    );
    let listed = references(&mut socket, BlockReferenceList::Orphans).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, block);
    assert_eq!(listed[0].references, 0);

    assert!(matches!(
        create(&mut socket, second, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    let listed = references(&mut socket, BlockReferenceList::References(block)).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, second);
    let listed = references(&mut socket, BlockReferenceList::Backrefs(second)).await;
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, block);
    server.cleanup().await;
}
