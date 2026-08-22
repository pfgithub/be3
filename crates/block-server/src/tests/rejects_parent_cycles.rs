use super::support::{create, set_parent, TestServer};
use block::{BlockParent, ErrorCode, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn rejects_parent_cycles() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, first, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut socket, second, vec![first]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, second, BlockParent::Uuid(first)).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, first, BlockParent::Uuid(second)).await,
        ServerMessage::Error {
            code: ErrorCode::ParentCycle,
            ..
        }
    ));
    server.cleanup().await;
}
