use super::support::{create_and_watch, post_presence, watch, TestServer};
use block::ServerMessage;
use uuid::Uuid;

#[tokio::test]
async fn presence_replays_existing_values_to_a_new_watcher() {
    let server = TestServer::start().await;
    let mut poster = server.connect().await;
    let id = Uuid::new_v4();
    assert!(matches!(
        create_and_watch(&mut poster, id).await,
        ServerMessage::Ok { .. }
    ));

    let presence_id = Uuid::new_v4();
    assert!(matches!(
        post_presence(&mut poster, id, presence_id, vec![1, 2, 3]).await,
        ServerMessage::Ok { .. }
    ));

    let mut watcher = server.connect().await;
    assert!(matches!(
        watch(&mut watcher, id).await,
        ServerMessage::ReadBlock { .. }
    ));

    let message = super::next_message(&mut watcher).await;
    let ServerMessage::Presence {
        id: block_id,
        presence_id: found_presence_id,
        data,
        ..
    } = message
    else {
        panic!("expected a presence replay, got {message:?}");
    };
    assert_eq!(block_id, id);
    assert_eq!(found_presence_id, presence_id);
    assert_eq!(data, Some(vec![1, 2, 3]));

    server.cleanup().await;
}
