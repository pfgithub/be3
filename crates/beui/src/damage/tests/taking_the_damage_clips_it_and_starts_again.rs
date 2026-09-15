use super::*;

#[test]
fn taking_the_damage_clips_it_and_starts_again() {
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    let mut damage = Damage::default();
    damage.add(rect(-40.0, 50.0, 60.0, 400.0));

    assert_eq!(damage.take(viewport), rect(0.0, 50.0, 60.0, 100.0));
    assert!(!damage.take(viewport).is_positive());
}
