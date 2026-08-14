use super::{CheckoutOutcome, Fixture};

#[tokio::test]
async fn version_control_checkout_leaves_unchanged_entries_untouched() {
    let fixture = Fixture::set_up().await;
    let changed_member = fixture.add_member().await;
    let stable_member = fixture.add_member().await;
    let first = fixture.commit("first").await;

    fixture.change_member(changed_member).await;
    fixture.commit("second").await;

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
    assert_ne!(replaced[0], changed_member);

    assert_eq!(
        fixture.live_id_of(fixture.eternal_id_of(stable_member)),
        Some(stable_member)
    );

    fixture.tear_down().await;
}
