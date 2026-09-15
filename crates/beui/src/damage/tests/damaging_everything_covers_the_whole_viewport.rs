use super::*;

#[test]
fn damaging_everything_covers_the_whole_viewport() {
    let viewport = rect(0.0, 0.0, 100.0, 100.0);
    let mut damage = Damage::default();
    damage.add(rect(10.0, 10.0, 20.0, 20.0));
    damage.everything();

    assert_eq!(damage.take(viewport), viewport);
}
