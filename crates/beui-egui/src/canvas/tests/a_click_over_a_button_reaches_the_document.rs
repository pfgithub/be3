use super::*;

#[test]
fn a_click_over_a_button_reaches_the_document() {
    let mut harness = harness();
    let center = button_center(&harness);

    click(&mut harness, center);

    assert_eq!(clicks(&harness), 1);
}
