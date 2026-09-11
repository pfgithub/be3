use super::*;
use crate::reactive::view;
use crate::styled::ListboxBuilder;

#[test]
fn listbox_typeahead_matches_prefixes_and_cycles_repeated_letters() {
    let (document, [listbox, after]) = toolbar_of(|| {
        [
            view! {
                <listbox labels={vec!["Apple".to_string(), "Banana".to_string(), "Blueberry".to_string(), "Cherry".to_string()]} selected={Some(0)} />
            },
            view! { <labelled_button label={"After".to_string()} /> },
        ]
    });
    let after_focus = unstyled::button_focused(&document, after);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("bl");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(2)
    );
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.type_text("b");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(1)
    );
    harness.type_text("b");
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(2)
    );
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(3)
    );
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(0)
    );
}
