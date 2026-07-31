use super::*;

#[tokio::test]
async fn sequence_errors_include_the_expected_sequence() {
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
                Some(4),
                Uuid::new_v4(),
                Uuid::new_v4(),
                vec![],
                "Block".into(),
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
