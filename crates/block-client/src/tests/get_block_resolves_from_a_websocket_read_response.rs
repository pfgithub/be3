use std::{sync::mpsc, thread, time::Duration};

use block::{
    Block, BlockParent, ClientMessage, CommandKind, OperationRecord, ReferenceDelta, ServerMessage,
};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};
use uuid::Uuid;

use super::{
    lib_test_support::{counter_operation, counter_snapshot, Counter},
    BlockClient,
};

#[tokio::test]
async fn get_block_resolves_from_a_websocket_read_response() {
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
                let ClientMessage::ReadBlock {
                    request_id,
                    id: requested_id,
                    ..
                } = request
                else {
                    panic!("expected read request");
                };
                assert_eq!(requested_id, id);
                socket
                    .send(Message::Text(
                        serde_json::to_string(&ServerMessage::ReadBlock {
                            request_id,
                            command: CommandKind::ReadBlock,
                            id,
                            block_type: Counter::TYPE_ID,
                            author: Uuid::new_v4(),
                            snapshot: counter_snapshot(2),
                            snapshot_seq: 0,
                            operations: vec![OperationRecord {
                                seq: 1,
                                operation_id: Uuid::new_v4(),
                                author: Uuid::new_v4(),
                                operation: counter_operation(3),
                                references: ReferenceDelta::default(),
                            }],
                            parent: BlockParent::Root,
                            name: "Counter 5".into(),
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
    assert!(block.read().is_none());
    client.connect(format!("ws://{address}"));
    tokio::time::timeout(Duration::from_secs(2), block.loaded())
        .await
        .unwrap();
    assert_eq!(block.read().unwrap().count, 5);

    drop(block);
    drop(client);
    server.join().unwrap();
}
