use super::*;

#[tokio::test]
async fn explicit_sequences_cannot_be_applied_out_of_order() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let id = Uuid::new_v4();
    store
        .create_block_unlocked(
            id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![],
            "Block".into(),
            vec![],
        )
        .await
        .unwrap();

    assert!(matches!(
        store
            .update_block_unlocked(
                id,
                Some(2),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await,
        Err(StoreError::InvalidSeq {
            expected: 1,
            actual: 2
        })
    ));
    store
        .update_block_unlocked(
            id,
            Some(1),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![1],
            "Block".into(),
            ReferenceDelta::default(),
        )
        .await
        .unwrap();
    store
        .update_block_unlocked(
            id,
            Some(2),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![2],
            "Block".into(),
            ReferenceDelta::default(),
        )
        .await
        .unwrap();

    let read = store.read_block_unlocked(id).await.unwrap();
    assert_eq!(
        read.operations
            .iter()
            .map(|record| record.seq)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
