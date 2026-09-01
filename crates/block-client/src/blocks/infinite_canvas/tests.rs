use std::{collections::BTreeMap, path::PathBuf};

use block::{Block, BlockParent};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::*;
use crate::block_ref::WorktreeMembership;
use crate::blocks::database::DatabaseValue;
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{BlockClient, ManagementClient};

mod direct_editor_serialization_round_trips;
mod direct_editor_transform_constraints_are_enforced;
mod infinite_canvas_applies_entity_changes;
mod infinite_canvas_block_reference_is_excluded_and_resolves_to_the_target;
mod infinite_canvas_block_reference_resolves_to_none_when_unresolvable;
mod infinite_canvas_exact_order_preserves_unlisted_slots;
mod infinite_canvas_history_undoes_and_redoes_add;
mod infinite_canvas_history_undoes_and_redoes_preview_region;
mod infinite_canvas_reorders_layers;
mod infinite_canvas_serialization_round_trips;
mod infinite_canvas_sets_preview_region;
mod infinite_canvas_tracks_block_references;
mod rebase_entity_preserves_conflicting_remote_fields;

fn block_entity(id: Uuid, block_id: BlockRef) -> CanvasEntity {
    CanvasEntity { id,
    transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(1.0, 1.0), 0.0),
    kind: CanvasEntityKind::Block { block_id },
    style: CanvasEntityStyle::default(),
    group_id: None,
    locked: false, components: Vec::new() }
}

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root = std::env::temp_dir().join(format!(
            "block-client-infinite-canvas-test-{}",
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
            "infinite-canvas-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}


fn direct_editor_entity(id: Uuid, block_id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(
            CanvasPoint::default(),
            CanvasPoint::new(1.0, 1.0),
            0.0,
        ),
        kind: CanvasEntityKind::DirectEditor {
            block_id: BlockRef::Direct(block_id),
            scale: 1.0,
        },
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}
mod replacing_a_component_schema_merges_into_an_existing_target;
mod component_block_values_deduplicate_and_rewrite_direct_references;
