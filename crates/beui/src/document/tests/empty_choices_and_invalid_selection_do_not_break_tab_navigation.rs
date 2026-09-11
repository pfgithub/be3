use super::*;
use crate::reactive::view;
use crate::styled::{ListboxBuilder, TabsBuilder};

#[test]
fn empty_choices_and_invalid_selection_do_not_break_tab_navigation() {
    let (document, [empty, tabs, after]) = toolbar_of(|| {
        [
            view! { <listbox labels={Vec::<String>::new()} selected={Some(4)} /> },
            view! { <tabs labels={vec!["One".to_string(), "Two".to_string()]} selected={99} /> },
            view! { <labelled_button label={"After".to_string()} /> },
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
