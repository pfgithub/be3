use super::support::{
    access_for, add_member, create, create_workspace, list_access, register, set_access,
    set_parent, update, TestServer,
};
use block::{BlockAccess, BlockParent, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn access_flows_down_to_owned_children_and_up_to_parents() {
    let server = TestServer::start().await;
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
    let mut socket = server.connect_to(owner.id, workspace.id).await;

    // grandparent -> parent -> child, all owned, plus a plain reference the
    // parent points at without owning.
    let grandparent = Uuid::new_v4();
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();
    let referenced = Uuid::new_v4();
    create(&mut socket, referenced, vec![]).await;
    create(&mut socket, child, vec![]).await;
    create(&mut socket, parent, vec![child, referenced]).await;
    create(&mut socket, grandparent, vec![parent]).await;
    set_parent(&mut socket, grandparent, BlockParent::Root).await;
    set_parent(&mut socket, parent, BlockParent::Uuid(grandparent)).await;
    set_parent(&mut socket, child, BlockParent::Uuid(parent)).await;

    set_access(&mut socket, parent, editor.id, BlockAccess::View).await;

    // Viewing the parent reaches its owned child at the same level.
    let entries = list_access(&mut socket, child).await;
    assert_eq!(access_for(&entries, editor.id), BlockAccess::View);

    // The parent's plain reference is only known to exist.
    let entries = list_access(&mut socket, referenced).await;
    assert_eq!(access_for(&entries, editor.id), BlockAccess::KnowExists);

    // Ancestors become findable so the shared block can be located.
    let entries = list_access(&mut socket, grandparent).await;
    assert_eq!(access_for(&entries, editor.id), BlockAccess::KnowExists);

    // The grandparent is only known to exist, so the editor cannot write to it.
    let mut editor_socket = server.connect_to(editor.id, workspace.id).await;
    assert!(matches!(
        update(&mut editor_socket, grandparent, vec![], vec![]).await,
        block::ServerMessage::Error {
            code: block::ErrorCode::PermissionDenied,
            ..
        }
    ));

    // Editing the parent carries all the way down to the owned child.
    set_access(&mut socket, parent, editor.id, BlockAccess::Edit).await;
    let entries = list_access(&mut socket, child).await;
    assert_eq!(access_for(&entries, editor.id), BlockAccess::Edit);
    assert!(matches!(
        update(&mut editor_socket, child, vec![], vec![]).await,
        block::ServerMessage::Ok { .. }
    ));
    server.cleanup().await;
}
