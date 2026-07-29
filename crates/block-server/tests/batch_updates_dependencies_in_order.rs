mod support;

use block::{BlockUpdate, ClientMessage, ReferenceDelta, ServerMessage};
use support::{create, read, relationships, request, TestServer};
use uuid::Uuid;

#[tokio::test]
async fn batch_updates_apply_reference_deltas_in_request_order() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let target = Uuid::new_v4();

    for id in [target, first, second] {
        assert!(matches!(
            create(&mut socket, id, vec![]).await,
            ServerMessage::Ok { .. }
        ));
    }
    let response = request(
        &mut socket,
        ClientMessage::UpdateBatch {
            request_id: Uuid::new_v4(),
            updates: vec![
                BlockUpdate {
                    id: first,
                    seq: None,
                    operation_id: Uuid::new_v4(),
                    operation: vec![],
                    references: ReferenceDelta {
                        added: vec![target],
                        removed: vec![],
                    },
                },
                BlockUpdate {
                    id: second,
                    seq: None,
                    operation_id: Uuid::new_v4(),
                    operation: vec![],
                    references: ReferenceDelta {
                        added: vec![target],
                        removed: vec![],
                    },
                },
            ],
        },
    )
    .await;
    assert!(matches!(response, ServerMessage::BatchOk { .. }));

    let mut expected_backrefs = vec![first, second];
    expected_backrefs.sort_unstable();
    assert_eq!(
        relationships(read(&mut socket, target).await).2,
        expected_backrefs
    );
    server.cleanup().await;
}
