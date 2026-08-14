use crate::blocks::version_control_data::{empty_tree_hash, VersionControlData, MAIN_BRANCH};

use super::Fixture;

#[tokio::test]
async fn version_control_commit_worktree_with_no_members_creates_empty_tree_commit() {
    let fixture = Fixture::set_up().await;

    let outcome = fixture.commit("empty commit").await;

    assert_eq!(outcome.tree_hash, empty_tree_hash());
    assert!(outcome.branch_advanced);

    let data = fixture
        .client
        .get_block::<VersionControlData>(fixture.data_id);
    let state = data.read().unwrap();
    assert_eq!(state.commits().len(), 2);
    assert_eq!(state.branch_head(MAIN_BRANCH), Some(&outcome.commit));
    drop(state);

    let worktree = fixture
        .client
        .get_block::<crate::blocks::version_control_worktree::VersionControlWorktree>(
            fixture.worktree_id,
        );
    assert_eq!(
        worktree.read().unwrap().checked_out_commit(),
        &outcome.commit
    );

    fixture.tear_down().await;
}
