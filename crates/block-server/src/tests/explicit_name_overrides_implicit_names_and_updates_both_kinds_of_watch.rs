use super::support;
use super::support::{create, request, set_parent, TestServer};
use block::{
    BlockParent, BlockReferenceList, ClientMessage, CommandKind, ReferenceDelta, ServerMessage,
};
use futures_util::StreamExt;
use tokio::time::{timeout, Duration};
use uuid::Uuid;

#[tokio::test]
async fn explicit_name_overrides_implicit_names_and_updates_both_kinds_of_watch() {
    let server = TestServer::start().await;
    let mut mutator = server.connect().await;
    let mut watcher = server.connect().await;
    let id = Uuid::new_v4();
    assert!(matches!(
        create(&mut mutator, id, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut mutator, id, BlockParent::Root).await,
        ServerMessage::Ok { .. }
    ));

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
        request(
            &mut watcher,
            ClientMessage::ReadBlock {
                request_id: Uuid::new_v4(),
                id,
                watch: true,
            },
        )
        .await,
        ServerMessage::ReadBlock { .. }
    ));

    assert!(matches!(
        request(
            &mut mutator,
            ClientMessage::SetBlockName {
                request_id: Uuid::new_v4(),
                id,
                name: "Explicit".into(),
            },
        )
        .await,
        ServerMessage::Ok {
            command: CommandKind::SetBlockName,
            ..
        }
    ));

    let first = next_message(&mut watcher).await;
    let second = next_message(&mut watcher).await;
    assert!(
        matches!(&first, ServerMessage::BlockNameUpdated { name, .. } if name == "Explicit")
            || matches!(&second, ServerMessage::BlockNameUpdated { name, .. } if name == "Explicit")
    );
    assert!(
        matches!(&first, ServerMessage::ReferencesUpdated { blocks, .. } if blocks[0].name == "Explicit")
            || matches!(&second, ServerMessage::ReferencesUpdated { blocks, .. } if blocks[0].name == "Explicit")
    );

    assert!(matches!(
        request(
            &mut mutator,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id,
                seq: None,
                operation_id: Uuid::new_v4(),
                operation: vec![],
                implicit_name: "Changed implicit".into(),
                dynamic_artifact: false,
                references: ReferenceDelta::default(),
            },
        )
        .await,
        ServerMessage::Ok {
            command: CommandKind::UpdateBlock,
            ..
        }
    ));
    assert!(matches!(
        next_message(&mut watcher).await,
        ServerMessage::BlockUpdated { name, .. } if name == "Explicit"
    ));
    assert!(timeout(Duration::from_millis(100), watcher.next())
        .await
        .is_err());

    server.cleanup().await;
}

async fn next_message(socket: &mut support::Socket) -> ServerMessage {
    let message = socket.next().await.unwrap().unwrap();
    serde_json::from_str(&message.into_text().unwrap()).unwrap()
}
