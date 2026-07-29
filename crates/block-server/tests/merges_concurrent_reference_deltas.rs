mod support;

use block::ServerMessage;
use support::{create, read, relationships, update, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn merges_reference_deltas_from_concurrent_clients() {
    let server = TestServer::start().await;
    let mut first_socket = server.connect().await;
    let mut second_socket = server.connect().await;
    let source = Uuid::new_v4();
    let first_target = Uuid::new_v4();
    let second_target = Uuid::new_v4();

    for id in [first_target, second_target, source] {
        assert!(matches!(
            create(&mut first_socket, id, vec![]).await,
            ServerMessage::Ok { .. }
        ));
    }
    let (first, second) = tokio::join!(
        update(&mut first_socket, source, vec![first_target], vec![]),
        update(&mut second_socket, source, vec![second_target], vec![])
    );
    assert!(matches!(first, ServerMessage::Ok { .. }));
    assert!(matches!(second, ServerMessage::Ok { .. }));

    let mut expected = vec![first_target, second_target];
    expected.sort_unstable();
    assert_eq!(
        relationships(read(&mut first_socket, source).await).1,
        expected
    );
    server.cleanup().await;
}
