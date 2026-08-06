use std::collections::BTreeMap;

use super::support::{create, references, request, TestServer};
use block::{BlockReferenceList, ClientMessage, ReferenceDelta, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn preserves_reference_order() {
    let server = TestServer::start().await;
    let mut socket = server.connect().await;
    let source = Uuid::new_v4();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    for id in [first, second] {
        assert!(matches!(
            create(&mut socket, id, vec![]).await,
            ServerMessage::Ok { .. }
        ));
    }
    assert!(matches!(
        create(&mut socket, source, vec![second, first]).await,
        ServerMessage::Ok { .. }
    ));
    assert_eq!(
        references(&mut socket, BlockReferenceList::References(source))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![second, first]
    );

    assert!(matches!(
        request(
            &mut socket,
            ClientMessage::UpdateBlock {
                request_id: Uuid::new_v4(),
                id: source,
                seq: None,
                operation_id: Uuid::new_v4(),
                operation: vec![],
                properties: BTreeMap::new(),
                dynamic_artifact: false,
                references: ReferenceDelta {
                    before: vec![second, first],
                    after: vec![first, second],
                },
            },
        )
        .await,
        ServerMessage::Ok { .. }
    ));
    assert_eq!(
        references(&mut socket, BlockReferenceList::References(source))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![first, second]
    );

    server.cleanup().await;
}
