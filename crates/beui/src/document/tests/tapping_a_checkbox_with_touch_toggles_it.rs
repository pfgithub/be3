use super::*;
use crate::reactive::view;
use crate::styled::CheckboxBuilder;

#[test]
fn tapping_a_checkbox_with_touch_toggles_it() {
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <checkbox label="Touch option" checked=false />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let center = harness.center(checkbox);

    harness.touch(TouchPhase::Start, center);
    harness.touch(TouchPhase::End, center);
    harness.frame(Vec::new());

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
