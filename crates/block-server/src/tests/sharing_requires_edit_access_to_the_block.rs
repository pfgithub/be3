use super::support::{
    add_member, create, create_workspace, list_access, register, set_access, TestServer,
};
use block::{BlockAccess, ErrorCode, ServerMessage, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn sharing_requires_edit_access_to_the_block() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let viewer = register(&management, "viewer@example.com").await;
    let stranger = register(&management, "stranger@example.com").await;
    let workspace = create_workspace(&management, &owner.token, "Shared").await;
    for account in [&viewer, &stranger] {
        add_member(
            &management,
            &owner.token,
            workspace.id,
            account,
            WorkspaceRole::Editor,
        )
        .await;
    }

    let mut owner_socket = server.connect_to(&owner.token, workspace.id).await;
    let block = Uuid::new_v4();
    create(&mut owner_socket, block, vec![]).await;
    set_access(&mut owner_socket, block, viewer.id, BlockAccess::View).await;

    // Being able to read a block is not enough to hand it on to others.
    let mut viewer_socket = server.connect_to(&viewer.token, workspace.id).await;
    assert!(matches!(
        set_access(&mut viewer_socket, block, stranger.id, BlockAccess::View).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));
    assert!(matches!(
        super::support::request(
            &mut viewer_socket,
            block::ClientMessage::ListBlockAccess {
                request_id: Uuid::new_v4(),
                id: block,
            },
        )
        .await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));

    // Once they can edit it, they can share it on.
    set_access(&mut owner_socket, block, viewer.id, BlockAccess::Edit).await;
    assert!(matches!(
        set_access(&mut viewer_socket, block, stranger.id, BlockAccess::View).await,
        ServerMessage::Ok { .. }
    ));
    let entries = list_access(&mut viewer_socket, block).await;
    assert_eq!(
        super::support::access_for(&entries, stranger.id),
        BlockAccess::View
    );
    server.cleanup().await;
}
