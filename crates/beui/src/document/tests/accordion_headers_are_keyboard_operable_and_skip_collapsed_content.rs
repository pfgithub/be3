use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::AccordionBuilder;

#[test]
fn accordion_headers_are_keyboard_operable_and_skip_collapsed_content() {
    let child = NodeRef::new();
    let (document, [accordion, after]) = toolbar_of({
        let child = child.clone();
        move || {
            [
                view! {
                    <accordion title={"Options".to_string()} open={false}>
                        <labelled_button node_ref={&child} label={"Child".to_string()} />
                    </accordion>
                },
                view! { <labelled_button label={"After".to_string()} /> },
            ]
        }
    });
    let child_focus = unstyled::button_focused(&document, child.get());
    let after_focus = unstyled::button_focused(&document, after);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
    assert!(!child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Enter, Modifiers::NONE);
    assert!(styled::accordion_open(harness.document(), accordion));
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(child_focus.get());
    harness.key(Key::Tab, Modifiers::SHIFT);
    harness.key(Key::Space, Modifiers::NONE);
    assert!(!styled::accordion_open(harness.document(), accordion));
}
