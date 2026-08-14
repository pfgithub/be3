use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_creates_members_missing_from_current_worktree() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let first = fixture.commit("with member").await;
    let eternal_id = fixture.eternal_id_of(member_id);

    let back_to_initial = fixture
        .checkout(fixture.initial_commit.clone(), false)
        .await;
    assert!(matches!(back_to_initial, CheckoutOutcome::Applied { .. }));
    assert_eq!(fixture.member_count(), 0);

    let outcome = fixture.checkout(first.commit.clone(), false).await;

    let CheckoutOutcome::Applied {
        created,
        replaced,
        detached,
    } = outcome
    else {
        panic!("expected checkout to be applied");
    };
    assert_eq!(created.len(), 1);
    assert!(replaced.is_empty());
    assert!(detached.is_empty());

    let new_id = created[0];
    assert_eq!(fixture.live_id_of(eternal_id), Some(new_id));
    assert_eq!(fixture.checked_out_commit(), first.commit);

    fixture.tear_down().await;
}
