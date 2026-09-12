use super::*;
use crate::reactive::view;
use crate::styled::Checkbox;

#[test]
fn enter_toggles_the_focused_checkbox() {
    let (document, [checkbox]) =
        toolbar_of(|| [view! { <Checkbox label="Show timings" checked=false /> }]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
