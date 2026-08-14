use super::Fixture;

#[tokio::test]
async fn version_control_checkout_worktree_with_changed_member_is_dirty() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    fixture.commit("first").await;

    fixture.change_member(member_id).await;

    assert!(!fixture.is_clean().await);

    fixture.tear_down().await;
}
