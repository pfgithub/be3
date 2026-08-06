use super::support::{create_and_watch, post_presence, watch, TestServer};
use block::ServerMessage;
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn presence_is_broadcast_to_other_watchers_but_not_the_poster() {
    let server = TestServer::start().await;
    let mut poster = server.connect().await;
    let mut watcher = server.connect().await;
    let id = Uuid::new_v4();
    assert!(matches!(
        create_and_watch(&mut poster, id).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        watch(&mut watcher, id).await,
        ServerMessage::ReadBlock { .. }
    ));

    let presence_id = Uuid::new_v4();
    assert!(matches!(
        post_presence(&mut poster, id, presence_id, vec![9]).await,
        ServerMessage::Ok { .. }
    ));

    let message = super::next_message(&mut watcher).await;
    let ServerMessage::Presence {
        id: block_id,
        presence_id: found_presence_id,
        data,
        ..
    } = message
    else {
        panic!("expected a presence update, got {message:?}");
    };
    assert_eq!(block_id, id);
    assert_eq!(found_presence_id, presence_id);
    assert_eq!(data, Some(vec![9]));

    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), poster.next())
            .await
            .is_err(),
        "the poster must not receive its own presence update"
    );

    server.cleanup().await;
}
