use crate::blocks::workspace_index::WorkspaceIndex;

use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_discards_and_recreates_changed_entries() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let first = fixture.commit("first").await;

    fixture.change_member(member_id).await;
    fixture.commit("second").await;

    fixture.change_member(member_id).await;
    assert!(!fixture.is_clean().await);

    let outcome = fixture.checkout(first.commit.clone(), true).await;

    let CheckoutOutcome::Applied {
        created,
        replaced,
        detached,
    } = outcome
    else {
        panic!("expected checkout to be applied");
    };
    assert!(created.is_empty());
    assert!(detached.is_empty());
    assert_eq!(replaced.len(), 1);
    let new_id = replaced[0];
    assert_ne!(new_id, member_id);

    assert_eq!(fixture.checked_out_commit(), first.commit);
    assert_eq!(
        fixture.live_id_of(fixture.eternal_id_of(new_id)),
        Some(new_id)
    );

    let recreated = fixture.client.get_block::<WorkspaceIndex>(new_id);
    recreated.loaded().await;
    assert!(recreated.read().unwrap().entries().is_empty());

    fixture.tear_down().await;
}
