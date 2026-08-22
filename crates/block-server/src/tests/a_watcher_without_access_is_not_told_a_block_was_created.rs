use std::time::Duration;

use super::support::{add_member, create, create_workspace, register, watch, TestServer};
use block::{ErrorCode, ServerMessage, WorkspaceRole};
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn a_watcher_without_access_is_not_told_a_block_was_created() {
    let server = TestServer::start().await;
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
    let mut owner_socket = server.connect_to(&owner.token, workspace.id).await;
    let mut editor_socket = server.connect_to(&editor.token, workspace.id).await;
    let id = Uuid::new_v4();

    assert!(matches!(
        watch(&mut editor_socket, id).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));
    assert!(matches!(
        create(&mut owner_socket, id, vec![]).await,
        ServerMessage::Ok { .. }
    ));

    assert!(
        tokio::time::timeout(Duration::from_millis(100), editor_socket.next())
            .await
            .is_err()
    );
    server.cleanup().await;
}
