use super::support::{
    access_for, add_member, create, create_workspace, list_access, register, set_access, TestServer,
};
use block::{BlockAccess, WorkspaceRole};
use uuid::Uuid;

#[tokio::test]
async fn block_access_survives_a_server_restart() {
    let server = TestServer::start().await;
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

    let mut socket = server.connect_to(owner.id, workspace.id).await;
    let block = Uuid::new_v4();
    create(&mut socket, block, vec![]).await;
    set_access(&mut socket, block, editor.id, BlockAccess::View).await;
    let root = server.stop().await;

    let server = TestServer::start_at_as(root, owner.id, workspace.id).await;
    let mut socket = server.connect().await;
    let entries = list_access(&mut socket, block).await;
    assert_eq!(access_for(&entries, editor.id), BlockAccess::View);
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.account.id == editor.id)
            .unwrap()
            .granted,
        Some(BlockAccess::View)
    );
    server.cleanup().await;
}
