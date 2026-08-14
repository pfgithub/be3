use crate::blocks::version_control_data::VersionControlData;

use super::Fixture;

#[tokio::test]
async fn version_control_commit_worktree_recommit_after_change_creates_new_object_and_commit() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;

    let first = fixture.commit("first").await;
    let objects_after_first = fixture.objects_len();

    fixture.change_member(member_id).await;
    fixture.client.synchronized().await;

    let second = fixture.commit("second").await;
    let objects_after_second = fixture.objects_len();

    assert!(objects_after_second > objects_after_first);
    assert_ne!(second.tree_hash, first.tree_hash);
    assert_eq!(fixture.commits_len(), 3);

    let data = fixture
        .client
        .get_block::<VersionControlData>(fixture.data_id);
    let state = data.read().unwrap();
    assert_eq!(
        state.commit(&second.commit).unwrap().parent.as_ref(),
        Some(&first.commit)
    );

    fixture.tear_down().await;
}
