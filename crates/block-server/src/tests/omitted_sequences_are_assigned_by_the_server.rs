use super::*;

#[tokio::test]
async fn omitted_sequences_are_assigned_by_the_server() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let (account, _token) = store
        .register_account(
            "omitted@example.com".into(),
            "Omitted".into(),
            TEST_PASSWORD.into(),
        )
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Omitted".into())
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
            BTreeMap::new(),
            false,
            vec![],
        )
        .await
        .unwrap();

    let first = store
        .update_block_unlocked(
            workspace.id,
            id,
            None,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![1],
            BTreeMap::new(),
            false,
            ReferenceDelta::default(),
        )
        .await
        .unwrap();
    let second = store
        .update_block_unlocked(
            workspace.id,
            id,
            None,
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![2],
            BTreeMap::new(),
            false,
            ReferenceDelta::default(),
        )
        .await
        .unwrap();

    assert!(matches!(first, UpdateOutcome::Inserted(record, _) if record.seq == 1));
    assert!(matches!(second, UpdateOutcome::Inserted(record, _) if record.seq == 2));
    drop(store);
    fs::remove_dir_all(root).await.unwrap();
}
