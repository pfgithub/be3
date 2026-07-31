use super::support::{create, request, set_parent, TestServer};
use block::{BlockParent, BlockReferenceList, ClientMessage, ServerMessage};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn reference_watch_updates_when_a_listed_blocks_parent_changes() {
    let server = TestServer::start().await;
    let mut writer = server.connect().await;
    let mut watcher = server.connect().await;
    let target = Uuid::new_v4();
    let source = Uuid::new_v4();
    let parent = Uuid::new_v4();

    create(&mut writer, target, vec![]).await;
    create(&mut writer, source, vec![target]).await;
    create(&mut writer, parent, vec![source]).await;

    assert!(matches!(
        request(
            &mut watcher,
            ClientMessage::ListReferences {
                request_id: Uuid::new_v4(),
                list: BlockReferenceList::Backrefs(target),
                watch: true,
            },
        )
        .await,
        ServerMessage::References { .. }
    ));
    assert!(matches!(
        set_parent(&mut writer, source, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));

    let message = watcher.next().await.unwrap().unwrap();
    let message: ServerMessage = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    assert!(matches!(
        message,
        ServerMessage::ReferencesUpdated { blocks, .. }
            if blocks.len() == 1 && blocks[0].parent == BlockParent::Uuid(parent)
    ));

    server.cleanup().await;
}
