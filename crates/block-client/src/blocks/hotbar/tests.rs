use std::path::PathBuf;

use block::BlockParent;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use super::{Hotbar, HotbarOperation, HotbarSlot};
use crate::block_ref::{BlockRef, WorktreeMembership};
use crate::blocks::version_control_data::VersionControlData;
use crate::blocks::version_control_worktree::{
    VersionControlWorktree, VersionControlWorktreeMembership,
};
use crate::blocks::workspace_index::WorkspaceIndex;
use crate::{BlockClient, ManagementClient};

fn component(name: &str, compiled: Uuid) -> HotbarSlot {
    HotbarSlot::Component {
        name: name.to_owned(),
        compiled: BlockRef::Direct(compiled),
    }
}

mod hotbar_component_reference_is_excluded_and_resolves_to_the_target;
mod hotbar_component_reference_resolves_to_none_when_unresolvable;
mod hotbar_history_restores_the_previous_layout;
mod hotbar_references_components_nested_in_folders;

struct TestServer {
    root: PathBuf,
    url: String,
    handle: JoinHandle<()>,
}

impl TestServer {
    async fn spawn() -> Self {
        let root =
            std::env::temp_dir().join(format!("block-client-hotbar-test-{}", Uuid::new_v4()));
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
            "hotbar-block-test-password",
        )
        .await
        .unwrap();
    let workspace = management
        .create_workspace(&session.token, "Test")
        .await
        .unwrap();
    (session.account.id, session.token, workspace.id)
}
