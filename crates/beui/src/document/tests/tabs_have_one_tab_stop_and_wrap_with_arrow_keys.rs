use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::TabsBuilder;

#[test]
fn tabs_have_one_tab_stop_and_wrap_with_arrow_keys() {
    let mut document = Document::new();
    let before = labelled_button(&mut document, "Before");
    let tabs = with_reactive_scope(&mut document, || {
        view! { <tabs labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string()]} selected={1} /> }
    });
    let after = labelled_button(&mut document, "After");
    let after_focus = unstyled::button_focused(&document, after);
    toolbar(&mut document, &[before, tabs, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 2);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 2);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 2);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 2);
}
