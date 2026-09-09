use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::{ListboxBuilder, TabsBuilder};

#[test]
fn empty_choices_and_invalid_selection_do_not_break_tab_navigation() {
    let mut document = Document::new();
    let (empty, tabs) = with_reactive_scope(&mut document, || {
        (
            view! { <listbox labels={Vec::<String>::new()} selected={Some(4)} /> },
            view! { <tabs labels={vec!["One".to_string(), "Two".to_string()]} selected={99} /> },
        )
    });
    assert_eq!(styled::tabs_selected(&document, tabs), 1);
    assert_eq!(styled::listbox_selected(&document, empty), None);
    let after = labelled_button(&mut document, "After");
    let after_focus = unstyled::button_focused(&document, after);
    toolbar(&mut document, &[empty, tabs, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
