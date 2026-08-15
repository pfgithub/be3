use std::path::PathBuf;

use block::{Block, BlockParent};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::*;
use crate::block_ref::WorktreeMembership;
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{BlockClient, ManagementClient};

mod presentation_allows_repeated_block_references;
mod presentation_clamps_insert_indices;
mod presentation_history_undoes_and_redoes_a_block_id_change;
mod presentation_history_undoes_and_redoes_changes;
mod presentation_history_undoes_grouped_removal;
mod presentation_ignores_duplicate_occurrence_ids;
mod presentation_inserts_removes_and_moves_slides;
mod presentation_references_are_deduplicated;
mod presentation_serialization_round_trips;
mod presentation_sets_slide_block_id;
mod presentation_slide_reference_is_excluded_and_resolves_to_the_target;
mod presentation_slide_reference_resolves_to_none_when_unresolvable;

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root =
            std::env::temp_dir().join(format!("block-client-presentation-test-{}", Uuid::new_v4()));
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
            "presentation-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
