use super::*;

#[tokio::test]
async fn operation_ids_are_idempotent_and_conflicts_are_rejected() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let account = store
        .register_account("operation@example.com".into(), "Operation".into())
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Operation".into())
        .await
        .unwrap();
    let id = Uuid::new_v4();
    store
        .create_block_unlocked(
            workspace.id,
            id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![1],
            "Block".into(),
            false,
            vec![],
        )
        .await
        .unwrap();
    let operation_id = Uuid::new_v4();

    assert!(matches!(
        store
            .update_block_unlocked(
                workspace.id,
                id,
                Some(1),
                operation_id,
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                false,
                ReferenceDelta::default(),
            )
            .await
            .unwrap(),
        UpdateOutcome::Inserted(..)
    ));
    assert!(matches!(
        store
            .update_block_unlocked(
                workspace.id,
                id,
                Some(99),
                operation_id,
                Uuid::new_v4(),
                vec![2],
                "Block".into(),
                false,
                ReferenceDelta::default(),
            )
            .await
            .unwrap(),
        UpdateOutcome::Duplicate(..)
    ));
    assert!(matches!(
        store
            .update_block_unlocked(
                workspace.id,
                id,
                Some(2),
                operation_id,
                Uuid::new_v4(),
                vec![3],
                "Block".into(),
                false,
                ReferenceDelta::default(),
            )
            .await,
        Err(StoreError::ConflictingOperationId)
    ));
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
