use super::*;

#[tokio::test]
async fn reads_replay_contiguous_operation_records() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let account = store
        .register_account("reads@example.com".into(), "Reads".into())
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Reads".into())
        .await
        .unwrap();
    let id = Uuid::new_v4();
    let block_type = Uuid::new_v4();
    store
        .create_block_unlocked(
            workspace.id,
            id,
            block_type,
            Uuid::new_v4(),
            vec![1],
            "Block".into(),
            false,
            vec![],
        )
        .await
        .unwrap();
    store
        .update_block_unlocked(
            workspace.id,
            id,
            Some(1),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![2],
            "Block".into(),
            false,
            ReferenceDelta::default(),
        )
        .await
        .unwrap();
    store
        .update_block_unlocked(
            workspace.id,
            id,
            Some(2),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![3],
            "Block".into(),
            false,
            ReferenceDelta::default(),
        )
        .await
        .unwrap();

    let read = store.read_block_unlocked(workspace.id, id).await.unwrap();
    assert_eq!(read.block_type, block_type);
    assert_eq!(read.snapshot, vec![1]);
    assert_eq!(read.snapshot_seq, 0);
    assert_eq!(read.operations.len(), 2);
    assert_eq!(read.operations[0].seq, 1);
    assert_eq!(read.operations[1].seq, 2);
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
