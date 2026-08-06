use super::support::{clear_presence, create_and_watch, post_presence, watch, TestServer};
use block::ServerMessage;
use uuid::Uuid;

#[tokio::test]
async fn presence_can_be_cleared_explicitly() {
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
    post_presence(&mut poster, id, presence_id, vec![7]).await;
    let posted = super::next_message(&mut watcher).await;
    assert!(matches!(
        posted,
        ServerMessage::Presence { data: Some(_), .. }
    ));

    assert!(matches!(
        clear_presence(&mut poster, id, presence_id).await,
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
