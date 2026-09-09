use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SwitchBuilder;

#[test]
fn space_toggles_checkboxes_switches_and_toggle_buttons() {
    let mut document = Document::new();
    let checkbox = styled::checkbox(&mut document, "Check", false);
    let switch = with_reactive_scope(&mut document, || view! { <switch on={false} /> });
    let toggle = styled::toggle_button(&mut document, "Bold", false);
    toolbar(&mut document, &[checkbox, switch, toggle]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(styled::checkbox_checked(harness.document(), checkbox));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(styled::switch_on(harness.document(), switch));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(styled::toggle_button_pressed(harness.document(), toggle));
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(!styled::toggle_button_pressed(harness.document(), toggle));
}
