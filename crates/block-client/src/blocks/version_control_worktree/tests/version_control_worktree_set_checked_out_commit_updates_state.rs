use uuid::Uuid;

use super::{
    apply, author, VersionControlData, VersionControlWorktree, VersionControlWorktreeOperation,
};

#[test]
fn version_control_worktree_set_checked_out_commit_updates_state() {
    let data = VersionControlData::new(author(), 1_000);
    let initial_commit = data.branch_head(super::MAIN_BRANCH).unwrap().clone();
    let mut worktree = VersionControlWorktree::new(Uuid::from_u128(0xc5), &data);
    assert_eq!(worktree.checked_out_commit(), &initial_commit);

    let other_commit = {
        let other = VersionControlData::new(author(), 2_000);
        other.branch_head(super::MAIN_BRANCH).unwrap().clone()
    };
    assert_ne!(other_commit, initial_commit);

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::SetCheckedOutCommit {
            commit: other_commit.clone(),
        },
    );

    assert_eq!(worktree.checked_out_commit(), &other_commit);
}
