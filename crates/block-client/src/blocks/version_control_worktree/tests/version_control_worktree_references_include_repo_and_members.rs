use block::Block;
use uuid::Uuid;

use super::{
    apply, author, VersionControlData, VersionControlWorktree, VersionControlWorktreeOperation,
};

#[test]
fn version_control_worktree_references_include_repo_and_members() {
    let repo = Uuid::from_u128(0xc6);
    let data = VersionControlData::new(author(), 1_000);
    let mut worktree = VersionControlWorktree::new(repo, &data);
    let live_id = Uuid::from_u128(0x1);
    let eternal_id = Uuid::from_u128(0x2);

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::AddMember {
            live_id,
            eternal_id,
        },
    );

    assert_eq!(worktree.references(), vec![repo, live_id]);
}
