use super::*;

#[tokio::test]
async fn resolve_reference_direct_returns_the_id_unchanged() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let target = Uuid::new_v4();

    let resolved = client
        .resolve_reference(
            Uuid::new_v4(),
            &BlockRef::Direct(target),
            &FakeWorktreeMembership,
        )
        .await;

    assert_eq!(resolved, Some(target));
}
