use std::{collections::HashMap, path::PathBuf};

use block::{Block, BlockParent};
use serde::{Deserialize, Serialize};
use tokio::{net::TcpListener, task::JoinHandle};
use uuid::Uuid;

use crate::{BlockClient, ManagementClient};

use super::{BlockRef, WorktreeMembership};

mod classify_reference_direct_across_worktrees;
mod classify_reference_direct_outside_a_worktree;
mod classify_reference_repo_relative_for_a_shared_worktree;
mod resolve_reference_direct_returns_the_id_unchanged;
mod resolve_reference_repo_relative_looks_up_the_eternal_id;
mod resolve_reference_unresolvable_on_repo_mismatch;

#[derive(Clone, Deserialize, Serialize)]
struct FakeWorktree {
    repo: Uuid,
    children: Vec<Uuid>,
    members: HashMap<Uuid, Uuid>,
}

#[derive(Clone, Deserialize, Serialize)]
enum FakeWorktreeOperation {
    AddChild(Uuid),
    AddMember { live_id: Uuid, eternal_id: Uuid },
}

impl Block for FakeWorktree {
    type Operation = FakeWorktreeOperation;
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(101);

    fn apply_operation(block: &mut Self, operation: &Self::Operation) {
        match operation {
            FakeWorktreeOperation::AddChild(id) => {
                if !block.children.contains(id) {
                    block.children.push(*id);
                }
            }
            FakeWorktreeOperation::AddMember {
                live_id,
                eternal_id,
            } => {
                block.members.insert(*live_id, *eternal_id);
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        self.children.clone()
    }
}

#[derive(Clone, Deserialize, Serialize)]
struct Member;

impl Block for Member {
    type Operation = ();
    type History = block::NoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(102);

    fn apply_operation(_block: &mut Self, _operation: &Self::Operation) {}
}

struct FakeWorktreeMembership;

impl WorktreeMembership for FakeWorktreeMembership {
    fn worktree_type_id(&self) -> Uuid {
        FakeWorktree::TYPE_ID
    }

    fn repo_id(&self, client: &BlockClient, worktree_id: Uuid) -> Option<Uuid> {
        Some(client.get_block::<FakeWorktree>(worktree_id).read()?.repo)
    }

    fn eternal_id_for_member(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        live_id: Uuid,
    ) -> Option<Uuid> {
        client
            .get_block::<FakeWorktree>(worktree_id)
            .read()?
            .members
            .get(&live_id)
            .copied()
    }

    fn mint_eternal_id(&self, client: &BlockClient, worktree_id: Uuid, live_id: Uuid) -> Uuid {
        let eternal_id = Uuid::new_v4();
        client
            .get_block::<FakeWorktree>(worktree_id)
            .operate(FakeWorktreeOperation::AddMember {
                live_id,
                eternal_id,
            });
        eternal_id
    }

    fn resolve_eternal_id(
        &self,
        client: &BlockClient,
        worktree_id: Uuid,
        eternal_id: Uuid,
    ) -> Option<Uuid> {
        client
            .get_block::<FakeWorktree>(worktree_id)
            .read()?
            .members
            .iter()
            .find_map(|(live, candidate)| (*candidate == eternal_id).then_some(*live))
    }
}

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root =
            std::env::temp_dir().join(format!("block-client-block-ref-test-{}", Uuid::new_v4()));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let server_root = root.clone();
        let handle = tokio::spawn(async move {
            block_server::serve(listener, server_root).await.unwrap();
        });
        Self { root, url, handle }
    }

    async fn shutdown(self) {
        self.handle.abort();
        let _ = self.handle.await;
        tokio::fs::remove_dir_all(self.root).await.unwrap();
    }
}

async fn identity(url: &str) -> (Uuid, String, Uuid) {
    let management = ManagementClient::new(url).unwrap();
    let session = management
        .register(
            format!("{}@example.com", Uuid::new_v4()),
            "Test",
            "block-ref-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
