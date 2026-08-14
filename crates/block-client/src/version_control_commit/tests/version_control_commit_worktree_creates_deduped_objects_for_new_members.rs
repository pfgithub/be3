use super::Fixture;

#[tokio::test]
async fn version_control_commit_worktree_creates_deduped_objects_for_new_members() {
    let fixture = Fixture::set_up().await;
    fixture.add_member().await;

    let outcome = fixture.commit("add member").await;

    assert!(outcome.branch_advanced);
    assert_eq!(fixture.objects_len(), 2);
    assert_eq!(fixture.commits_len(), 2);

    fixture.tear_down().await;
}
