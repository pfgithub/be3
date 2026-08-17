use super::*;

#[tokio::test]
async fn disabled_history() {
    let client = crate::BlockClient::new(uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    let counter = client.create_block(Counter::new());
    assert!(!counter.supports_history());
}
