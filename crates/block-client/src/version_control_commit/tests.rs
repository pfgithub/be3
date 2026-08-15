use std::path::PathBuf;

use block::{Block, BlockParent};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::version_control_data::{VersionControlData, MAIN_BRANCH};
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::{WorkspaceIndex, WorkspaceIndexOperation};
use crate::{BlockClient, ManagementClient};

use super::commit_worktree;

mod version_control_commit_worktree_creates_deduped_objects_for_new_members;
mod version_control_commit_worktree_recommit_after_change_creates_new_object_and_commit;
mod version_control_commit_worktree_recommit_without_changes_creates_no_new_objects;
mod version_control_commit_worktree_reports_rejected_branch_advance_without_blocking_commit;
mod version_control_commit_worktree_with_no_members_creates_empty_tree_commit;

fn author() -> Uuid {
    Uuid::from_u128(0x901d)
}

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root = std::env::temp_dir().join(format!(
            "block-client-version-control-commit-test-{}",
            Uuid::new_v4()
        ));
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
            "version-control-commit-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}

struct Fixture {
    server: TestServer,
    client: BlockClient,
    data_id: Uuid,
    worktree_id: Uuid,
}

impl Fixture {
    async fn set_up() -> Self {
        let server = TestServer::spawn().await;
        let (account_id, token, workspace_id) = identity(&server.url).await;
        let client = BlockClient::new(account_id, workspace_id);
        client.connect(server.url.clone(), token);

        let data_value = VersionControlData::new(account_id, 1_000);
        let data = client.create_block(data_value.clone());
        let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
        data.loaded().await;
        worktree.loaded().await;
        worktree.set_parent(BlockParent::Root);
        client.synchronized().await;

        assert_eq!(
            worktree.read().unwrap().checked_out_commit(),
            data_value.branch_head(MAIN_BRANCH).unwrap()
        );

        Self {
            server,
            client,
            data_id: data.id(),
            worktree_id: worktree.id(),
        }
    }

    async fn add_member(&self) -> Uuid {
        let member = self.client.create_block(WorkspaceIndex::default());
        member.loaded().await;
        let membership = VersionControlWorktreeMembership;
        membership.mint_eternal_id(&self.client, self.worktree_id, member.id());
        self.client.synchronized().await;
        member.set_parent(BlockParent::Uuid(self.worktree_id));
        self.client.synchronized().await;
        member.id()
    }

    async fn change_member(&self, member_id: Uuid) {
        let nested = self.client.create_block(WorkspaceIndex::default());
        nested.loaded().await;
        let member = self.client.get_block::<WorkspaceIndex>(member_id);
        member.operate(WorkspaceIndexOperation::Add(BlockRef::Direct(nested.id())));
        nested.set_parent(BlockParent::Uuid(member_id));
    }

    fn objects_len(&self) -> usize {
        self.client
            .get_block::<VersionControlData>(self.data_id)
            .read()
            .unwrap()
            .references()
            .len()
    }

    fn commits_len(&self) -> usize {
        self.client
            .get_block::<VersionControlData>(self.data_id)
            .read()
            .unwrap()
            .commits()
            .len()
    }

    async fn commit(&self, message: &str) -> super::CommitOutcome {
        commit_worktree(
            &self.client,
            self.worktree_id,
            author(),
            2_000,
            message.to_owned(),
        )
        .await
        .expect("commit_worktree should succeed")
    }

    async fn tear_down(self) {
        self.server.shutdown().await;
    }
}
