use super::*;

#[tokio::test]
async fn disabled_history() {
    let client = crate::BlockClient::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let tree = client.create_block(FileTree::new());
    assert!(!tree.supports_history());
}
