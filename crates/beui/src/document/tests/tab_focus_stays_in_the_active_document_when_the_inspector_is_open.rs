use super::*;
use crate::reactive::view;

#[test]
fn tab_focus_stays_in_the_active_document_when_the_inspector_is_open() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! { <LabelledButton label="First" /> },
            view! { <LabelledButton label="Second" /> },
        ]
    });
    let first_focused = unstyled::button_focused(&document, first);
    let second_focused = unstyled::button_focused(&document, second);
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.key(Key::Tab, Modifiers::NONE);

    assert_eq!((first_focused.get(), second_focused.get()), (true, false));
    assert_eq!(harness.inspector().document.focused_node(), None);

    let accesskit_tab = harness.accesskit_tab_center();
    harness.click(accesskit_tab);
    let inspector_focus = harness.inspector().document.focused_node();
    assert!(inspector_focus.is_some());
    assert_eq!(harness.document.focused_node(), None);

    harness.key(Key::Tab, Modifiers::NONE);

    assert_eq!(harness.document.focused_node(), None);
    assert_ne!(harness.inspector().document.focused_node(), inspector_focus);
}
