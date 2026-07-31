use super::support::{create, references, update, TestServer};
use block::{BlockReferenceList, ServerMessage};
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

    let references = references(&mut first_socket, BlockReferenceList::References(source))
        .await
        .into_iter()
        .map(|block| block.id)
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    assert!(references.contains(&first_target));
    assert!(references.contains(&second_target));
    server.cleanup().await;
}
