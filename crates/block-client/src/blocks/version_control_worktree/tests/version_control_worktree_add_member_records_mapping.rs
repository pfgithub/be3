use uuid::Uuid;

use super::{
    apply, author, VersionControlData, VersionControlWorktree, VersionControlWorktreeOperation,
};

#[test]
fn version_control_worktree_add_member_records_mapping() {
    let data = VersionControlData::new(author(), 1_000);
    let mut worktree = VersionControlWorktree::new(Uuid::from_u128(0xc2), &data);
    let live_id = Uuid::from_u128(0x1);
    let eternal_id = Uuid::from_u128(0x2);

    apply(
        &mut worktree,
        VersionControlWorktreeOperation::AddMember {
            live_id,
            eternal_id,
        },
    );

    assert_eq!(worktree.eternal_id_for_member(live_id), Some(eternal_id));
    assert_eq!(worktree.resolve_eternal_id(eternal_id), Some(live_id));
    assert_eq!(
        worktree.members().collect::<Vec<_>>(),
        vec![(eternal_id, live_id)]
    );
}
