use super::support::{
    add_member, create, create_workspace, read, references, register, set_access, set_parent,
    update, TestServer,
};
use block::{
    BlockAccess, BlockParent, BlockReferenceList, ErrorCode, ServerMessage, WorkspaceRole,
};
use uuid::Uuid;

#[tokio::test]
async fn editors_only_reach_blocks_they_authored_or_were_granted() {
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
    let private = Uuid::new_v4();
    create(&mut owner_socket, private, vec![]).await;
    set_parent(&mut owner_socket, private, BlockParent::Root).await;

    let mut editor_socket = server.connect_to(&editor.token, workspace.id).await;

    // The block is invisible until it is shared.
    assert!(references(&mut editor_socket, BlockReferenceList::Roots)
        .await
        .is_empty());
    assert!(matches!(
        read(&mut editor_socket, private).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));

    // Viewers see the block and can read it, but cannot change it.
    set_access(&mut owner_socket, private, editor.id, BlockAccess::View).await;
    let listed = references(&mut editor_socket, BlockReferenceList::Roots).await;
    assert_eq!(listed.len(), 1);
    // The listing says so too, so a client can tell before opening anything.
    assert_eq!(listed[0].access, BlockAccess::View);
    assert!(matches!(
        read(&mut editor_socket, private).await,
        ServerMessage::ReadBlock {
            access: BlockAccess::View,
            ..
        }
    ));
    assert!(matches!(
        update(&mut editor_socket, private, vec![], vec![]).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));

    // Granting edit lets the same account write.
    set_access(&mut owner_socket, private, editor.id, BlockAccess::Edit).await;
    assert!(matches!(
        read(&mut editor_socket, private).await,
        ServerMessage::ReadBlock {
            access: BlockAccess::Edit,
            ..
        }
    ));
    assert!(matches!(
        update(&mut editor_socket, private, vec![], vec![]).await,
        ServerMessage::Ok { .. }
    ));

    // Withdrawing the grant hides the block again.
    set_access(&mut owner_socket, private, editor.id, BlockAccess::None).await;
    assert!(matches!(
        read(&mut editor_socket, private).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));

    // Blocks the editor authors are always theirs to edit.
    let own = Uuid::new_v4();
    assert!(matches!(
        create(&mut editor_socket, own, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        read(&mut editor_socket, own).await,
        ServerMessage::ReadBlock { .. }
    ));
    server.cleanup().await;
}
