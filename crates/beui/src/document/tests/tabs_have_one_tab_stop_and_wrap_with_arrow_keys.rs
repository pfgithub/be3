use super::*;
use crate::reactive::view;
use crate::styled::TabsBuilder;

#[test]
fn tabs_have_one_tab_stop_and_wrap_with_arrow_keys() {
    let (document, [_before, tabs, after]) = toolbar_of(|| {
        [
            view! { <labelled_button label="Before" /> },
            view! { <tabs labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string()]} selected=1 /> },
            view! { <labelled_button label="After" /> },
        ]
    });
    let after_focus = unstyled::button_focused(&document, after);
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
