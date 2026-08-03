use super::support::{add_member, create_workspace, register, TestServer};
use block::WorkspaceRole;
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn block_connections_require_workspace_membership() {
    let server = TestServer::start().await;
    // Someone who is not a member is refused before the websocket handshake
    // completes.
    assert!(server
        .try_connect_to(Uuid::new_v4(), server.workspace_id)
        .await
        .is_err());

    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let editor = register(&management, "editor@example.com").await;
    let workspace = create_workspace(&management, owner.id, "Shared").await;
    add_member(
        &management,
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
