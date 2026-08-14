use uuid::Uuid;

use crate::blocks::version_control_data::{
    empty_tree_hash, Commit, VersionControlData, VersionControlDataOperation, MAIN_BRANCH,
};
use crate::blocks::version_control_worktree::VersionControlWorktree;

use super::Fixture;

#[tokio::test]
async fn version_control_commit_worktree_reports_rejected_branch_advance_without_blocking_commit() {
    let fixture = Fixture::set_up().await;

    let data = fixture
        .client
        .get_block::<VersionControlData>(fixture.data_id);
    let initial_commit = data
        .read()
        .unwrap()
        .branch_head(MAIN_BRANCH)
        .unwrap()
        .clone();

    let foreign_commit = Commit {
        parent: Some(initial_commit.clone()),
        tree_hash: empty_tree_hash(),
        author: Uuid::from_u128(0xf0e1_9999),
        time: 1_500,
        message: "someone else's commit".to_owned(),
    };
    let foreign_commit_id = foreign_commit.id();
    data.operate(VersionControlDataOperation::AppendCommit {
        parent: foreign_commit.parent.clone(),
        tree_hash: foreign_commit.tree_hash.clone(),
        author: foreign_commit.author,
        time: foreign_commit.time,
        message: foreign_commit.message.clone(),
    });
    data.operate(VersionControlDataOperation::SetBranch {
        name: MAIN_BRANCH.to_owned(),
        expected: Some(initial_commit.clone()),
        commit: foreign_commit_id.clone(),
    });
    fixture.client.synchronized().await;
    assert_eq!(
        data.read().unwrap().branch_head(MAIN_BRANCH),
        Some(&foreign_commit_id)
    );

    let outcome = fixture.commit("stale commit").await;

    assert!(!outcome.branch_advanced);
    assert_eq!(
        data.read().unwrap().branch_head(MAIN_BRANCH),
        Some(&foreign_commit_id)
    );
    assert_eq!(
        data.read().unwrap().commit(&outcome.commit).unwrap().parent,
        Some(initial_commit)
    );

    let worktree = fixture
        .client
        .get_block::<VersionControlWorktree>(fixture.worktree_id);
    assert_eq!(
        worktree.read().unwrap().checked_out_commit(),
        &outcome.commit
    );

    fixture.tear_down().await;
}
