use super::{author, VersionControlData, VersionControlWorktree, MAIN_BRANCH};

#[test]
fn version_control_worktree_new_checks_out_the_repos_initial_commit() {
    let repo = uuid::Uuid::from_u128(0xc1);
    let data = VersionControlData::new(author(), 1_000);
    let initial_commit = data.branch_head(MAIN_BRANCH).expect("main exists").clone();

    let worktree = VersionControlWorktree::new(repo, &data);

    assert_eq!(worktree.repo(), repo);
    assert_eq!(worktree.checked_out_commit(), &initial_commit);
    assert_eq!(worktree.members().count(), 0);
}
