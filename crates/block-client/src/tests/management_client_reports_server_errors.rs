use crate::{ManagementClient, ManagementClientError};
use block::ManagementErrorCode;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn management_client_reports_server_errors() {
    assert!(matches!(
        ManagementClient::new("https://example.com"),
        Err(ManagementClientError::InvalidUrl(_))
    ));
    let root = std::env::temp_dir().join(format!("block-client-management-{}", Uuid::new_v4()));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = ManagementClient::new(format!("ws://{}", listener.local_addr().unwrap())).unwrap();
    let server_root = root.clone();
    let server = tokio::spawn(async move {
        block_server::serve(listener, server_root).await.unwrap();
    });

    let error = client.login("missing@example.com").await.unwrap_err();
    assert!(matches!(
        error,
        ManagementClientError::Server {
            code: ManagementErrorCode::AccountNotFound,
            ..
        }
    ));

    server.abort();
    let _ = server.await;
    tokio::fs::remove_dir_all(root).await.unwrap();
}
