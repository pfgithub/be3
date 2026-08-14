use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_detaches_member_missing_from_target() {
    let fixture = Fixture::set_up().await;
    let member_id = fixture.add_member().await;
    let eternal_id = fixture.eternal_id_of(member_id);
    fixture.commit("with member").await;

    let outcome = fixture
        .checkout(fixture.initial_commit.clone(), false)
        .await;

    let CheckoutOutcome::Applied {
        created,
        replaced,
        detached,
    } = outcome
    else {
        panic!("expected checkout to be applied");
    };
    assert!(created.is_empty());
    assert!(replaced.is_empty());
    assert_eq!(detached, vec![member_id]);

    assert_eq!(fixture.live_id_of(eternal_id), None);
    assert_eq!(fixture.member_count(), 0);
    assert_eq!(fixture.checked_out_commit(), fixture.initial_commit);

    fixture.tear_down().await;
}
