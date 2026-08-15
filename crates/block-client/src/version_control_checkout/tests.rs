use std::path::PathBuf;

use block::BlockParent;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::version_control_data::{CommitId, VersionControlData, MAIN_BRANCH};
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::{WorkspaceIndex, WorkspaceIndexOperation};
use crate::version_control_commit::{commit_worktree, CommitOutcome};
use crate::{BlockClient, ManagementClient};

use super::{checkout_worktree, materialize_worktree, worktree_is_clean, CheckoutOutcome};

mod version_control_checkout_blocked_when_dirty_without_discard;
mod version_control_checkout_creates_members_missing_from_current_worktree;
mod version_control_checkout_detaches_member_missing_from_target;
mod version_control_checkout_discards_and_recreates_changed_entries;
mod version_control_checkout_leaves_unchanged_entries_untouched;
mod version_control_checkout_preserves_eternal_id_across_changed_entry;
mod version_control_checkout_second_worktree_materializes_existing_repo_content;
mod version_control_checkout_worktree_with_changed_member_is_dirty;
mod version_control_checkout_worktree_with_no_changes_is_clean;

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
            "block-client-version-control-checkout-test-{}",
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
            "version-control-checkout-test-password",
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
    initial_commit: CommitId,
}

impl Fixture {
    async fn set_up() -> Self {
        let server = TestServer::spawn().await;
        let (account_id, token, workspace_id) = identity(&server.url).await;
        let client = BlockClient::new(account_id, workspace_id);
        client.connect(server.url.clone(), token);

        let data_value = VersionControlData::new(account_id, 1_000);
        let initial_commit = data_value.branch_head(MAIN_BRANCH).unwrap().clone();
        let data = client.create_block(data_value.clone());
        let worktree = client.create_block(VersionControlWorktree::new(data.id(), &data_value));
        data.loaded().await;
        worktree.loaded().await;
        worktree.set_parent(BlockParent::Root);
        client.synchronized().await;

        Self {
            server,
            client,
            data_id: data.id(),
            worktree_id: worktree.id(),
            initial_commit,
        }
    }

    async fn add_member(&self) -> Uuid {
        self.add_member_to(self.worktree_id).await
    }

    async fn add_member_to(&self, worktree_id: Uuid) -> Uuid {
        let member = self.client.create_block(WorkspaceIndex::default());
        member.loaded().await;
        let membership = VersionControlWorktreeMembership;
        membership.mint_eternal_id(&self.client, worktree_id, member.id());
        self.client.synchronized().await;
        member.set_parent(BlockParent::Uuid(worktree_id));
        self.client.synchronized().await;
        member.id()
    }

    async fn change_member(&self, member_id: Uuid) {
        let nested = self.client.create_block(WorkspaceIndex::default());
        nested.loaded().await;
        let member = self.client.get_block::<WorkspaceIndex>(member_id);
        member.operate(WorkspaceIndexOperation::Add(BlockRef::Direct(nested.id())));
        nested.set_parent(BlockParent::Uuid(member_id));
        self.client.synchronized().await;
    }

    async fn commit(&self, message: &str) -> CommitOutcome {
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

    async fn is_clean(&self) -> bool {
        worktree_is_clean(&self.client, self.worktree_id)
            .await
            .expect("worktree_is_clean should succeed")
    }

    async fn checkout(&self, target: CommitId, discard: bool) -> CheckoutOutcome {
        checkout_worktree(&self.client, self.worktree_id, target, discard)
            .await
            .expect("checkout_worktree should succeed")
    }

    fn checked_out_commit(&self) -> CommitId {
        self.client
            .get_block::<VersionControlWorktree>(self.worktree_id)
            .read()
            .unwrap()
            .checked_out_commit()
            .clone()
    }

    fn eternal_id_of(&self, live_id: Uuid) -> Uuid {
        self.client
            .get_block::<VersionControlWorktree>(self.worktree_id)
            .read()
            .unwrap()
            .eternal_id_for_member(live_id)
            .unwrap()
    }

    fn live_id_of(&self, eternal_id: Uuid) -> Option<Uuid> {
        self.client
            .get_block::<VersionControlWorktree>(self.worktree_id)
            .read()
            .unwrap()
            .resolve_eternal_id(eternal_id)
    }

    fn member_count(&self) -> usize {
        self.client
            .get_block::<VersionControlWorktree>(self.worktree_id)
            .read()
            .unwrap()
            .members()
            .count()
    }

    async fn tear_down(self) {
        self.server.shutdown().await;
    }
}
