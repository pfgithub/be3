use super::support::{create_and_watch, post_presence, unwatch, watch, TestServer};
use block::ServerMessage;
use uuid::Uuid;

#[tokio::test]
async fn presence_is_cleared_when_a_client_unwatches() {
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
    post_presence(&mut poster, id, presence_id, vec![3]).await;
    let posted = super::next_message(&mut watcher).await;
    assert!(matches!(
        posted,
        ServerMessage::Presence { data: Some(_), .. }
    ));

    assert!(matches!(
        unwatch(&mut poster, id).await,
        ServerMessage::Ok { .. }
    ));

    let cleared = super::next_message(&mut watcher).await;
    let ServerMessage::Presence {
        id: block_id,
        presence_id: found_presence_id,
        data,
        ..
    } = cleared
    else {
        panic!("expected a presence clear, got {cleared:?}");
    };
    assert_eq!(block_id, id);
    assert_eq!(found_presence_id, presence_id);
    assert_eq!(data, None);

    server.cleanup().await;
}
