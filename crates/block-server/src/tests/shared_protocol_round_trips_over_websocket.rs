use super::*;

#[tokio::test]
async fn shared_protocol_round_trips_over_websocket() {
    let root = test_root();
    let store = Arc::new(BlockStore::new(root.clone()));
    let account = store
        .register_account("protocol@example.com".into(), "Protocol".into())
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Protocol".into())
        .await
        .unwrap();
    let watch_hub = Arc::new(WatchHub::new());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = tokio::spawn({
        let store = Arc::clone(&store);
        let watch_hub = Arc::clone(&watch_hub);
        async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, store, watch_hub).await.unwrap();
        }
    });
    let mut client = test_connect(format!("ws://{addr}"), account.id, workspace.id).await;
    let id = Uuid::new_v4();
    let block_type = Uuid::new_v4();

    let create_request = Uuid::new_v4();
    send_message(
        &mut client,
        ClientMessage::CreateBlock {
            request_id: create_request,
            id,
            block_type,
            data: vec![1, 2],
            implicit_name: "Block".into(),
            dynamic_artifact: false,
            references: vec![],
            watch: true,
        },
    )
    .await;
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::Ok {
            request_id,
            command: CommandKind::CreateBlock,
            ..
        } if request_id == create_request
    ));

    let operation_id = Uuid::new_v4();
    send_message(
        &mut client,
        ClientMessage::UpdateBlock {
            request_id: Uuid::new_v4(),
            id,
            seq: Some(1),
            operation_id,
            operation: vec![3],
            implicit_name: "Block".into(),
            dynamic_artifact: false,
            references: ReferenceDelta::default(),
        },
    )
    .await;
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::Ok {
            command: CommandKind::UpdateBlock,
            seq: Some(1),
            operation_id: Some(found),
            ..
        } if found == operation_id
    ));
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::BlockUpdated {
            operation: OperationRecord {
                seq: 1,
                operation_id: found,
                ..
            },
            ..
        } if found == operation_id
    ));

    let read_request = Uuid::new_v4();
    send_message(
        &mut client,
        ClientMessage::ReadBlock {
            request_id: read_request,
            id,
            watch: true,
        },
    )
    .await;
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::ReadBlock {
            request_id,
            block_type: found_type,
            snapshot_seq: 0,
            operations,
            ..
        } if request_id == read_request
            && found_type == block_type
            && operations.len() == 1
            && operations[0].operation_id == operation_id
    ));

    client.close(None).await.unwrap();
    server.await.unwrap();
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
