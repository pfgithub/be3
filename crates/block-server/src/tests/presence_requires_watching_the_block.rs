use super::support::{create_and_watch, post_presence, unwatch, TestServer};
use block::{ErrorCode, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn presence_requires_watching_the_block() {
    let server = TestServer::start().await;
    let mut client = server.connect().await;
    let id = Uuid::new_v4();
    assert!(matches!(
        create_and_watch(&mut client, id).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        unwatch(&mut client, id).await,
        ServerMessage::Ok { .. }
    ));

    let response = post_presence(&mut client, id, Uuid::new_v4(), vec![1]).await;
    assert!(matches!(
        response,
        ServerMessage::Error {
            code: ErrorCode::NotWatching,
            ..
        }
    ));

    server.cleanup().await;
}
