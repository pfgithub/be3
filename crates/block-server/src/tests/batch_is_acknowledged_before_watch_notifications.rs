use super::*;

#[tokio::test]
async fn batch_is_acknowledged_before_watch_notifications() {
    let root = test_root();
    let store = Arc::new(BlockStore::new(root.clone()));
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
    let mut client = test_connect(format!("ws://{addr}")).await;
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    for id in [first, second] {
        send_message(
            &mut client,
            ClientMessage::CreateBlock {
                request_id: Uuid::new_v4(),
                id,
                block_type: Uuid::new_v4(),
                data: vec![],
                implicit_name: "Block".into(),
                references: vec![],
                watch: true,
            },
        )
        .await;
        assert!(matches!(
            next_message(&mut client).await,
            ServerMessage::Ok {
                command: CommandKind::CreateBlock,
                ..
            }
        ));
    }

    let request_id = Uuid::new_v4();
    send_message(
        &mut client,
        ClientMessage::UpdateBatch {
            request_id,
            updates: vec![
                block::BlockUpdate {
                    id: first,
                    seq: Some(1),
                    operation_id: Uuid::new_v4(),
                    operation: vec![1],
                    implicit_name: "First".into(),
                    references: ReferenceDelta::default(),
                },
                block::BlockUpdate {
                    id: second,
                    seq: Some(1),
                    operation_id: Uuid::new_v4(),
                    operation: vec![2],
                    implicit_name: "Second".into(),
                    references: ReferenceDelta::default(),
                },
            ],
        },
    )
    .await;
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::BatchOk {
            request_id: found,
            operations,
            ..
        } if found == request_id && operations.len() == 2
    ));
    assert!(matches!(
        next_message(&mut client).await,
        ServerMessage::BatchUpdated { operations }
            if operations.len() == 2
    ));

    client.close(None).await.unwrap();
    server.await.unwrap();
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
