mod support;

use block::{BlockReferenceList, ClientMessage, ServerMessage};
use futures_util::StreamExt;
use support::{create, request, update, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn reference_watch_updates_when_a_listed_blocks_reference_count_changes() {
    let server = TestServer::start().await;
    let mut writer = server.connect().await;
    let mut watcher = server.connect().await;
    let target = Uuid::new_v4();
    let source = Uuid::new_v4();

    create(&mut writer, target, vec![]).await;
    create(&mut writer, source, vec![]).await;

    assert!(matches!(
        request(
            &mut watcher,
            ClientMessage::ListReferences {
                request_id: Uuid::new_v4(),
                list: BlockReferenceList::Roots,
                watch: true,
            },
        )
        .await,
        ServerMessage::References { .. }
    ));
    assert!(matches!(
        update(&mut writer, source, vec![target], vec![]).await,
        ServerMessage::Ok { .. }
    ));

    let message = watcher.next().await.unwrap().unwrap();
    let message: ServerMessage = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    assert!(matches!(
        message,
        ServerMessage::ReferencesUpdated { blocks, .. }
            if blocks.iter().any(|block| block.id == source && block.references == 1)
    ));

    server.cleanup().await;
}
