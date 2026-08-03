use super::support::{
    access_for, add_member, create, create_workspace, list_access, read, references, register,
    set_access, set_parent, TestServer,
};
use block::{
    BlockAccess, BlockParent, BlockReferenceList, ErrorCode, ServerMessage, WorkspaceRole,
};
use uuid::Uuid;

#[tokio::test]
async fn administrators_reach_every_block_without_grants() {
    let server = TestServer::start().await;
    let management = server.management();
    let owner = register(&management, "owner@example.com").await;
    let editor = register(&management, "editor@example.com").await;
    let second_admin = register(&management, "admin@example.com").await;
    let workspace = create_workspace(&management, owner.id, "Shared").await;
    add_member(
        &management,
        owner.id,
        workspace.id,
        &editor,
        WorkspaceRole::Editor,
    )
    .await;
    add_member(
        &management,
        owner.id,
        workspace.id,
        &second_admin,
        WorkspaceRole::Administrator,
    )
    .await;

    // A block only the editor has ever touched.
    let mut editor_socket = server.connect_to(editor.id, workspace.id).await;
    let block = Uuid::new_v4();
    create(&mut editor_socket, block, vec![]).await;
    set_parent(&mut editor_socket, block, BlockParent::Root).await;

    let mut admin_socket = server.connect_to(second_admin.id, workspace.id).await;
    assert!(matches!(
        read(&mut admin_socket, block).await,
        ServerMessage::ReadBlock { .. }
    ));
    assert_eq!(
        references(&mut admin_socket, BlockReferenceList::Roots)
            .await
            .len(),
        1
    );

    let entries = list_access(&mut admin_socket, block).await;
    assert_eq!(access_for(&entries, owner.id), BlockAccess::Edit);
    assert_eq!(access_for(&entries, second_admin.id), BlockAccess::Edit);

    // Recording a grant for an administrator would be overridden by the role,
    // so the server refuses rather than storing something meaningless.
    assert!(matches!(
        set_access(&mut admin_socket, block, owner.id, BlockAccess::View).await,
        ServerMessage::Error {
            code: ErrorCode::PermissionDenied,
            ..
        }
    ));
    server.cleanup().await;
}
