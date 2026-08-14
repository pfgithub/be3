use crate::blocks::version_control_data::VersionControlData;

use super::Fixture;

#[tokio::test]
async fn version_control_commit_worktree_recommit_without_changes_creates_no_new_objects() {
    let fixture = Fixture::set_up().await;
    fixture.add_member().await;

    let first = fixture.commit("first").await;
    let objects_after_first = fixture.objects_len();

    let second = fixture.commit("second").await;
    let objects_after_second = fixture.objects_len();

    assert_eq!(objects_after_first, objects_after_second);
    assert_eq!(fixture.commits_len(), 3);
    assert_eq!(second.tree_hash, first.tree_hash);
    assert_ne!(second.commit, first.commit);

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
