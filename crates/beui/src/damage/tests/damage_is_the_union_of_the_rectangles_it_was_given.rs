use super::*;

#[test]
fn damage_is_the_union_of_the_rectangles_it_was_given() {
    let mut damage = Damage::default();
    damage.add(rect(10.0, 10.0, 20.0, 20.0));
    damage.add(rect(40.0, 5.0, 50.0, 30.0));
    damage.add(Rect::NOTHING);

    assert_eq!(
        damage.take(rect(0.0, 0.0, 100.0, 100.0)),
        rect(10.0, 5.0, 50.0, 30.0)
    );
}
