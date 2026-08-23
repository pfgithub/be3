use super::support::{add_member, create_workspace, register, TestServer};
use block::WorkspaceRole;
use futures_util::StreamExt;

#[tokio::test]
async fn block_connections_require_workspace_membership() {
    let server = TestServer::start().await;

    assert!(server
        .try_connect_to("not-a-real-token", server.workspace_id)
        .await
        .is_err());

    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let editor = register(&management, "editor@example.com").await;
    let workspace = create_workspace(&management, &owner.token, "Shared").await;
    add_member(
        &management,
        &owner.token,
        workspace.id,
        &editor,
        WorkspaceRole::Editor,
    )
    .await;

    let mut socket = server.connect_to(&editor.token, workspace.id).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(250), socket.next())
            .await
            .is_err()
    );
    server.cleanup().await;
}
