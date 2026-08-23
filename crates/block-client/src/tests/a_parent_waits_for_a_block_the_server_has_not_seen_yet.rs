use std::{collections::BTreeMap, sync::mpsc, thread, time::Duration};

use block::{
    Block, BlockAccess, BlockParent, ClientMessage, CommandKind, ErrorCode, ServerMessage,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use super::{
    lib_test_support::{counter_snapshot, Counter},
    BlockClient,
};

#[tokio::test]
async fn a_parent_waits_for_a_block_the_server_has_not_seen_yet() {
    let id = Uuid::new_v4();
    let author = Uuid::new_v4();
    let (address_tx, address_rx) = mpsc::channel();
    let (parented_tx, parented_rx) = mpsc::channel();
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
                let quiet = tokio::time::timeout(Duration::from_millis(250), socket.next()).await;
                assert!(
                    quiet.is_err(),
                    "the client asked the server to parent a block it does not have"
                );
                socket
                    .send(Message::Text(
                        serde_json::to_string(&ServerMessage::BlockCreated {
                            id,
                            block_type: Counter::TYPE_ID,
                            author,
                            snapshot: counter_snapshot(3),
                            snapshot_seq: 0,
                            parent: BlockParent::Orphaned,
                            properties: BTreeMap::new(),
                            access: BlockAccess::Edit,
                        })
                        .unwrap(),
                    ))
                    .await
                    .unwrap();
                let request = socket.next().await.unwrap().unwrap();
                let request: ClientMessage =
                    serde_json::from_str(&request.into_text().unwrap()).unwrap();
                let ClientMessage::SetBlockParent {
                    request_id,
                    id: parented,
                    parent,
                } = request
                else {
                    panic!("expected the parent to be set once the block existed");
                };
                assert_eq!(parented, id);
                assert_eq!(parent, BlockParent::Root);
                socket
                    .send(Message::Text(
                        serde_json::to_string(&ServerMessage::Ok {
                            request_id,
                            command: CommandKind::SetBlockParent,
                            id,
                            seq: None,
                            operation_id: None,
                        })
                        .unwrap(),
                    ))
                    .await
                    .unwrap();
                parented_tx.send(()).unwrap();
                while socket.next().await.is_some() {}
            });
    });

    let address = address_rx.recv().unwrap();
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let block = client.get_block::<Counter>(id);
    client.set_block_parent(id, BlockParent::Root);
    client.connect(format!("http://{address}"), "test-token");
    parented_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    tokio::time::timeout(Duration::from_secs(2), block.loaded())
        .await
        .unwrap();

    drop(block);
    drop(client);
    server.join().unwrap();
}
