use super::*;

#[tokio::test]
async fn explicit_sequences_cannot_be_applied_out_of_order() {
    let root = test_root();
    let store = BlockStore::new(root.clone());
    let (account, _token) = store
        .register_account(
            "sequence@example.com".into(),
            "Sequence".into(),
            TEST_PASSWORD.into(),
        )
        .await
        .unwrap();
    let workspace = store
        .create_workspace(account.id, "Sequence".into())
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

    assert!(matches!(
        store
            .update_block_unlocked(
                workspace.id,
                id,
                Some(2),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![2],
                BTreeMap::new(),
                false,
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
            workspace.id,
            id,
            Some(1),
            Uuid::new_v4(),
            Uuid::new_v4(),
            vec![1],
            BTreeMap::new(),
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
            vec![2],
            BTreeMap::new(),
            false,
            ReferenceDelta::default(),
        )
        .await
        .unwrap();

    let read = store.read_block_unlocked(workspace.id, id).await.unwrap();
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
