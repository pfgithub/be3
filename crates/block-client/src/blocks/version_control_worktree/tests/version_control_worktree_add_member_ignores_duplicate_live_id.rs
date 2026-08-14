use uuid::Uuid;

use super::{
    apply, author, VersionControlData, VersionControlWorktree, VersionControlWorktreeOperation,
};

#[test]
fn version_control_worktree_add_member_ignores_duplicate_live_id() {
    let data = VersionControlData::new(author(), 1_000);
    let mut worktree = VersionControlWorktree::new(Uuid::from_u128(0xc3), &data);
    let live_id = Uuid::from_u128(0x1);
    let first_eternal_id = Uuid::from_u128(0x2);
    let second_eternal_id = Uuid::from_u128(0x3);

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::AddMember {
            live_id,
            eternal_id: first_eternal_id,
        },
    );
    apply(
        &mut worktree,
        VersionControlWorktreeOperation::AddMember {
            live_id,
            eternal_id: second_eternal_id,
        },
    );

    assert_eq!(
        worktree.eternal_id_for_member(live_id),
        Some(first_eternal_id)
    );
    assert_eq!(worktree.resolve_eternal_id(second_eternal_id), None);
    assert_eq!(worktree.members().count(), 1);
}
