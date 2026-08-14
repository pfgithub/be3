use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_blocked_when_dirty_without_discard() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let first = fixture.commit("first").await;

    fixture.change_member(member_id).await;
    assert!(!fixture.is_clean().await);

    let outcome = fixture
        .checkout(fixture.initial_commit.clone(), false)
        .await;

    assert_eq!(outcome, CheckoutOutcome::Blocked);
    assert_eq!(fixture.checked_out_commit(), first.commit);
    assert_eq!(fixture.member_count(), 1);
    assert_eq!(
        fixture.live_id_of(fixture.eternal_id_of(member_id)),
        Some(member_id)
    );

    fixture.tear_down().await;
}
