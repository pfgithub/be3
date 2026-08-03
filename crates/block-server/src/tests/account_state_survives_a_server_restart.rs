use super::support::{management_request, TestServer};
use block::{ManagementClientMessage, ManagementServerMessage};
use uuid::Uuid;

#[tokio::test]
async fn account_state_survives_a_server_restart() {
    let server = TestServer::start().await;
    let mut socket = server.connect_management().await;
    let registered = management_request(
        &mut socket,
        ManagementClientMessage::Register {
            request_id: Uuid::new_v4(),
            email: "restart@example.com".into(),
            display_name: "Restart".into(),
        },
    )
    .await;
    let ManagementServerMessage::Account { account, .. } = registered else {
        panic!("registration failed: {registered:?}");
    };
    drop(socket);
    let root = server.stop().await;
    assert!(root.join("server.sqlite3").is_file());

    let restarted = TestServer::start_at(root).await;
    let mut socket = restarted.connect_management().await;
    let response = management_request(
        &mut socket,
        ManagementClientMessage::Login {
            request_id: Uuid::new_v4(),
            email: account.email.clone(),
        },
    )
    .await;
    assert!(matches!(
        response,
        ManagementServerMessage::Account { account: found, .. } if found == account
    ));
    restarted.cleanup().await;
}
