use super::*;

#[tokio::test]
async fn sequence_errors_include_the_expected_sequence() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let account = store
        .register_account("errors@example.com".into(), "Errors".into())
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Errors".into())
        .await
        .unwrap();
    let id = Uuid::new_v4();
    store
        .create_block_unlocked(
            workspace.id,
            id,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![],
            "Block".into(),
            false,
            vec![],
        )
        .await
        .unwrap();
    assert!(matches!(
        store
            .update_block_unlocked(
                workspace.id,
                id,
                Some(4),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![],
                "Block".into(),
                false,
                ReferenceDelta::default(),
            )
            .await,
        Err(StoreError::InvalidSeq {
            expected: 1,
            actual: 4
        })
    ));
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
