use super::*;
use crate::reactive::view;
use crate::styled::{CheckboxBuilder, SwitchBuilder, ToggleButtonBuilder};

#[test]
fn space_toggles_checkboxes_switches_and_toggle_buttons() {
    let (document, [checkbox, switch, toggle]) = toolbar_of(|| {
        [
            view! { <checkbox label={"Check".to_string()} checked={false} /> },
            view! { <switch on={false} /> },
            view! { <toggle_button label={"Bold".to_string()} pressed={false} /> },
        ]
    });
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
