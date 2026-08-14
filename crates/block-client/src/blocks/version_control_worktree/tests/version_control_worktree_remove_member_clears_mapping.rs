use uuid::Uuid;

use super::{
    apply, author, VersionControlData, VersionControlWorktree, VersionControlWorktreeOperation,
};

#[test]
fn version_control_worktree_remove_member_clears_mapping() {
    let data = VersionControlData::new(author(), 1_000);
    let mut worktree = VersionControlWorktree::new(Uuid::from_u128(0xc4), &data);
    let live_id = Uuid::from_u128(0x1);
    let eternal_id = Uuid::from_u128(0x2);
    apply(
        &mut worktree,
        VersionControlWorktreeOperation::AddMember {
            live_id,
            eternal_id,
        },
    );

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::RemoveMember { live_id },
    );

    assert_eq!(worktree.eternal_id_for_member(live_id), None);
    assert_eq!(worktree.resolve_eternal_id(eternal_id), None);
    assert_eq!(worktree.members().count(), 0);

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::RemoveMember { live_id },
    );
    assert_eq!(worktree.members().count(), 0);
}
