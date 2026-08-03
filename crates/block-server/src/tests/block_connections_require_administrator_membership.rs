use super::support::TestServer;
use futures_util::StreamExt;
use uuid::Uuid;

#[tokio::test]
async fn block_connections_require_administrator_membership() {
    let server = TestServer::start().await;
    let mut socket = server.connect_as(Uuid::new_v4()).await;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), socket.next())
            .await
            .is_ok()
    );
    server.cleanup().await;
}
