use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::CheckboxBuilder;

#[test]
fn enter_toggles_the_focused_checkbox() {
    let mut document = Document::new();
    let checkbox = with_reactive_scope(&mut document, || {
        view! { <checkbox label={"Show timings".to_string()} checked={false} /> }
    });
    toolbar(&mut document, &[checkbox]);
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert!(styled::checkbox_checked(harness.document(), checkbox));
}
