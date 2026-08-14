use crate::block_ref::WorktreeMembership;
use crate::blocks::version_control_worktree::VersionControlWorktreeMembership;

use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_preserves_eternal_id_across_changed_entry() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let first = fixture.commit("first").await;
    let eternal_id = fixture.eternal_id_of(member_id);

    fixture.change_member(member_id).await;
    fixture.commit("second").await;

    fixture.change_member(member_id).await;

    let CheckoutOutcome::Applied { replaced, .. } =
        fixture.checkout(first.commit.clone(), true).await
    else {
        panic!("expected checkout to be applied");
    };
    let new_id = replaced[0];

    let membership = VersionControlWorktreeMembership;
    assert_eq!(
        membership.resolve_eternal_id(&fixture.client, fixture.worktree_id, eternal_id),
        Some(new_id)
    );

    fixture.tear_down().await;
}
