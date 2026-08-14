use super::Fixture;

#[tokio::test]
async fn version_control_checkout_worktree_with_no_changes_is_clean() {
    let fixture = Fixture::set_up().await;
    fixture.add_member().await;
    fixture.commit("first").await;

    assert!(fixture.is_clean().await);

    fixture.tear_down().await;
}
