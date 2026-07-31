use super::*;

#[tokio::test]
async fn operation_ids_are_idempotent_and_conflicts_are_rejected() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let id = Uuid::new_v4();
    store
        .create_block_unlocked(
            id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![1],
            "Block".into(),
            vec![],
        )
        .await
        .unwrap();
    let operation_id = Uuid::new_v4();

    assert!(matches!(
        store
            .update_block_unlocked(
                id,
                Some(1),
                operation_id,
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap(),
        UpdateOutcome::Inserted(..)
    ));
    assert!(matches!(
        store
            .update_block_unlocked(
                id,
                Some(99),
                operation_id,
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await
            .unwrap(),
        UpdateOutcome::Duplicate(..)
    ));
    assert!(matches!(
        store
            .update_block_unlocked(
                id,
                Some(2),
                operation_id,
                Uuid::new_v4(),
                vec![3],
                "Block".into(),
                ReferenceDelta::default(),
            )
            .await,
        Err(StoreError::ConflictingOperationId)
    ));
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
