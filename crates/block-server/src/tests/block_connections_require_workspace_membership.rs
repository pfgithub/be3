use super::support::{add_member, create_workspace, register, TestServer};
use block::WorkspaceRole;
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn block_connections_require_workspace_membership() {
    let server = TestServer::start().await;
    let mut socket = server.connect_as(Uuid::new_v4()).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
            .await
            .is_ok()
    );

    let mut management = server.connect_management().await;
    let owner = register(&mut management, "owner@example.com").await;
    let editor = register(&mut management, "editor@example.com").await;
    let workspace = create_workspace(&mut management, owner.id, "Shared").await;
    add_member(
        &mut management,
        owner.id,
        workspace.id,
        &editor,
        WorkspaceRole::Editor,
    )
    .await;

    // Editors are members, so the handshake succeeds even though they cannot
    // reach any of the workspace's blocks yet.
    let mut socket = server.connect_to(editor.id, workspace.id).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(250), socket.next())
            .await
            .is_err()
    );
    server.cleanup().await;
}
