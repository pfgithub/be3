use super::support::{create, request, set_parent, TestServer};
use block::{BlockParent, BlockReferenceList, ClientMessage, ServerMessage};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn parent_watch_updates() {
    let server = TestServer::start().await;
    let mut writer = server.connect().await;
    let mut watcher = server.connect().await;
    let first_root = Uuid::new_v4();
    let second_root = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    for (id, references) in [
        (child, vec![]),
        (parent, vec![child]),
        (first_root, vec![parent]),
        (second_root, vec![parent]),
    ] {
        create(&mut writer, id, references).await;
    }
    set_parent(&mut writer, parent, BlockParent::Uuid(first_root)).await;
    set_parent(&mut writer, child, BlockParent::Uuid(parent)).await;

    assert!(matches!(
        request(
            &mut watcher,
            ClientMessage::ListReferences {
                request_id: Uuid::new_v4(),
                list: BlockReferenceList::Parents(child),
                watch: true,
            },
        )
        .await,
        ServerMessage::References { blocks, .. }
            if blocks.iter().map(|block| block.id).collect::<Vec<_>>()
                == vec![first_root, parent]
    ));

    assert!(matches!(
        set_parent(&mut writer, parent, BlockParent::Uuid(second_root)).await,
        ServerMessage::Ok { .. }
    ));

    let message = watcher.next().await.unwrap().unwrap();
    let message: ServerMessage = serde_json::from_str(&message.into_text().unwrap()).unwrap();
    assert!(matches!(
        message,
        ServerMessage::ReferencesUpdated { blocks, .. }
            if blocks.iter().map(|block| block.id).collect::<Vec<_>>()
                == vec![second_root, parent]
    ));

    server.cleanup().await;
}
