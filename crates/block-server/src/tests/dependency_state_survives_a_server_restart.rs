use super::support::{create, parent as read_parent, read, references, set_parent, TestServer};
use block::{BlockParent, BlockReferenceList, ServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn dependency_state_survives_a_server_restart() {
    let server = TestServer::start().await;
    let account_id = server.account_id;
    let workspace_id = server.workspace_id;
    let mut socket = server.connect().await;
    let parent = Uuid::new_v4();
    let child = Uuid::new_v4();

    assert!(matches!(
        create(&mut socket, child, vec![]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        create(&mut socket, parent, vec![child]).await,
        ServerMessage::Ok { .. }
    ));
    assert!(matches!(
        set_parent(&mut socket, child, BlockParent::Uuid(parent)).await,
        ServerMessage::Ok { .. }
    ));
    drop(socket);
    let root = server.stop().await;
    assert!(root.join("server.sqlite3").is_file());
    assert!(!root.join("dependencies.json").exists());
    assert!(!root.join(parent.to_string()).exists());
    assert!(!root.join(child.to_string()).exists());

    let restarted = TestServer::start_at_as(root, account_id, workspace_id).await;
    let mut socket = restarted.connect().await;
    assert_eq!(
        read_parent(read(&mut socket, child).await),
        BlockParent::Uuid(parent)
    );
    assert_eq!(
        references(&mut socket, BlockReferenceList::Backrefs(child))
            .await
            .into_iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![parent]
    );
    restarted.cleanup().await;
}
