use std::time::Duration;

use super::support::{create, request, update, TestServer};
use block::{BlockReferenceList, ClientMessage, ServerMessage};
use futures_util::StreamExt;
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
async fn watches_reference_changes_until_unwatched() {
    let server = TestServer::start().await;
    let mut writer = server.connect().await;
    let mut watcher = server.connect().await;
    let child = Uuid::new_v4();
    let source = Uuid::new_v4();

    create(&mut writer, child, vec![]).await;
    create(&mut writer, source, vec![child]).await;

    assert!(matches!(
        request(
            &mut watcher,
            ClientMessage::ListReferences {
                request_id: Uuid::new_v4(),
                list: BlockReferenceList::References(source),
                watch: true,
            },
        )
        .await,
        ServerMessage::References { blocks, .. }
            if blocks.iter().map(|block| block.id).collect::<Vec<_>>() == vec![child]
    ));

    update(&mut writer, source, vec![], vec![child]).await;
    let message = watcher.next().await.unwrap().unwrap();
    let message: ServerMessage = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    assert!(matches!(
        message,
        ServerMessage::ReferencesUpdated {
            list: BlockReferenceList::References(id),
            blocks,
        } if id == source && blocks.is_empty()
    ));

    assert!(matches!(
        request(
            &mut watcher,
            ClientMessage::UnwatchReferences {
                request_id: Uuid::new_v4(),
                list: BlockReferenceList::References(source),
            },
        )
        .await,
        ServerMessage::Ok { .. }
    ));
    update(&mut writer, source, vec![child], vec![]).await;
    assert!(timeout(Duration::from_millis(100), watcher.next())
        .await
        .is_err());

    server.cleanup().await;
}
