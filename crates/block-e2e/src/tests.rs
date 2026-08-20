use std::{future::Future, time::Duration};

use block::{Block, BlockParent, BlockReferenceList};
use block_client::{blocks::text::TextDocument, BlockClient, ManagementClient};
use serde::{Deserialize, Serialize};
use tokio::{fs, net::TcpListener};
use uuid::Uuid;

mod a_tunnelled_client_shares_the_hosts_connection;
mod batched_updates_are_observed_together;
mod client_orders_parent_assignment_after_creation_and_reference_updates;
mod crdt_text_clients_converge_after_concurrent_insertions;
mod real_clients_synchronize_through_the_real_server;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
struct Counter {
    count: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
enum CounterOperation {
    Add(i64),
}

impl Block for Counter {
    type Operation = CounterOperation;
    type History = block::NoHistory;
    const TYPE_ID: Uuid = Uuid::from_u128(1);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let CounterOperation::Add(amount) = operation;
        block.count += amount;
    }

    fn implicit_name(&self) -> Option<String> {
        Some(format!("Counter {}", self.count))
    }

    fn transform_operation(_local: &mut Self::Operation, _remote: &Self::Operation) {}
}

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
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7265_6665_7265_6e63_652d_7465_7374_0001);
    const CRDT: bool = true;

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        let ReferenceOperation::Add(id) = operation;
        if !block.references.contains(id) {
            block.references.push(*id);
        }
    }

    fn implicit_name(&self) -> Option<String> {
        Some("References".into())
    }

    fn references(&self) -> Vec<Uuid> {
        self.references.clone()
    }
}

async fn timeout(future: impl Future<Output = ()>) {
    tokio::time::timeout(Duration::from_secs(2), future)
        .await
        .unwrap();
}

async fn test_identity(url: &str) -> (Uuid, String, Uuid) {
    let management = ManagementClient::new(url).unwrap();
    let session = management
        .register(
            format!("{}@example.com", Uuid::new_v4()),
            "Test",
            "e2e-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
