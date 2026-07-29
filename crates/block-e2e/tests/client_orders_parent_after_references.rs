use block::{Block, BlockParent};
use block_client::BlockClient;
use serde::{Deserialize, Serialize};
use tokio::{fs, net::TcpListener};
use uuid::Uuid;

#[derive(Clone, Default, Deserialize, Serialize)]
struct ReferencingBlock {
    references: Vec<Uuid>,
}

#[derive(Clone, Deserialize, Serialize)]
enum ReferenceOperation {
    Add(Uuid),
}

impl Block for ReferencingBlock {
    type Operation = ReferenceOperation;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7265_6665_7265_6e63_652d_7465_7374_0001);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let ReferenceOperation::Add(id) = operation;
        if !block.references.contains(id) {
            block.references.push(*id);
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }
}

#[tokio::test]
async fn client_orders_parent_assignment_after_creation_and_reference_updates() {
    let root = std::env::temp_dir().join(format!("block-client-parent-test-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });

    let client = BlockClient::new();
    let parent = client.create_block(ReferencingBlock::default());
    let child = client.create_block(ReferencingBlock::default());
    client.connect(url);
    parent.loaded().await;
    child.loaded().await;

    parent.operate(ReferenceOperation::Add(child.id()));
    child.set_parent(BlockParent::Uuid(parent.id()));
    client.synchronized().await;

    assert_eq!(
        client
            .list_references(BlockParent::Root)
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![parent.id()]
    );

    drop(parent);
    drop(child);
    drop(client);
    server.abort();
    let _ = server.await;
    fs::remove_dir_all(root).await.unwrap();
}
