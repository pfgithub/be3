use super::*;
use crate::reactive::view;
use crate::styled::{Listbox, Tabs};

#[test]
fn empty_choices_and_invalid_selection_do_not_break_tab_navigation() {
    let (document, [empty, tabs, after]) = toolbar_of(|| {
        [
            view! { <Listbox labels={Vec::<String>::new()} selected=Some(4) /> },
            view! { <Tabs labels={vec!["One".to_string(), "Two".to_string()]} selected=99 /> },
            view! { <LabelledButton label="After" /> },
        ]
    });
    assert_eq!(styled::tabs_selected(&document, tabs), 1);
    assert_eq!(styled::listbox_selected(&document, empty), None);
    let after_focus = unstyled::button_focused(&document, after);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
