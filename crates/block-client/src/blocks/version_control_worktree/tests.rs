use std::path::PathBuf;

use block::{Block, BlockParent};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{BlockClient, ManagementClient};

use super::super::version_control_data::{VersionControlData, MAIN_BRANCH};
use super::{
    VersionControlWorktree, VersionControlWorktreeMembership, VersionControlWorktreeOperation,
};

mod version_control_worktree_add_member_ignores_duplicate_live_id;
mod version_control_worktree_add_member_records_mapping;
mod version_control_worktree_classify_and_resolve_reference_between_siblings;
mod version_control_worktree_membership_mints_and_resolves_eternal_ids;
mod version_control_worktree_new_checks_out_the_repos_initial_commit;
mod version_control_worktree_references_include_repo_and_members;
mod version_control_worktree_remove_member_clears_mapping;
mod version_control_worktree_set_checked_out_commit_updates_state;

fn apply(worktree: &mut VersionControlWorktree, operation: VersionControlWorktreeOperation) {
    VersionControlWorktree::apply_operation(worktree, &operation);
}

fn author() -> Uuid {
    Uuid::from_u128(0xb2)
}

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root = std::env::temp_dir().join(format!(
            "block-client-version-control-worktree-test-{}",
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
            "version-control-worktree-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
