use std::path::PathBuf;

use block::BlockParent;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{repo_relative_block_url, BlockClient, ManagementClient};

use super::*;

mod grouped_text_edits_are_sent_as_one_crdt_update;
mod text_embedded_repo_relative_reference_is_excluded_and_resolves_to_the_target;
mod text_embedded_repo_relative_reference_resolves_to_none_when_unresolvable;
mod text_history_restores_bytes_after_later_ids_change;
mod text_history_undoes_and_redoes_grouped_edits;
mod text_implicit_name_stops_at_newline_and_a_utf8_boundary;
mod text_indent_width_round_trips_through_operations_and_serialization;
mod text_item_ids_resolve_visible_characters;
mod text_language_round_trips_through_operations_and_serialization;
mod text_operations_are_crdt_updates_and_do_not_keep_a_confirmed_copy;
mod text_remove_item_operation_targets_stable_id;
mod text_serialization_preserves_invalid_utf8_bytes;
mod text_tracks_embedded_block_references;

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root = std::env::temp_dir().join(format!("block-client-text-test-{}", Uuid::new_v4()));
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
            "text-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
