use std::{sync::mpsc, thread, time::Duration};

use block::{BlockAccess, ClientMessage, CommandKind, ErrorCode, ServerMessage};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use super::{lib_test_support::Counter, BlockClient};

#[tokio::test]
async fn a_refused_read_leaves_the_block_unresolved() {
    let id = Uuid::new_v4();
    let (address_tx, address_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                address_tx.send(listener.local_addr().unwrap()).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let mut socket = accept_async(stream).await.unwrap();
                let request = socket.next().await.unwrap().unwrap();
                let request: ClientMessage =
                    serde_json::from_str(&request.into_text().unwrap()).unwrap();
                let ClientMessage::ReadBlock { request_id, .. } = request else {
                    panic!("expected read request");
                };
                socket
                    .send(Message::Text(
                        serde_json::to_string(&ServerMessage::Error {
                            request_id: Some(request_id),
                            command: Some(CommandKind::ReadBlock),
                            id: Some(id),
                            code: ErrorCode::PermissionDenied,
                            message: "no access".into(),
                            expected_seq: None,
                        })
                        .unwrap(),
                    ))
                    .await
                    .unwrap();
                while socket.next().await.is_some() {}
            });
    });

    let address = address_rx.recv().unwrap();
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.get_block::<Counter>(id);
    client.connect(format!("http://{address}"), "test-token");
    tokio::time::timeout(Duration::from_secs(2), client.synchronized())
        .await
        .unwrap();
    assert!(block.read().is_none());
    assert_eq!(client.block_access(id), BlockAccess::KnowExists);

    drop(block);
    drop(client);
    server.join().unwrap();
}
