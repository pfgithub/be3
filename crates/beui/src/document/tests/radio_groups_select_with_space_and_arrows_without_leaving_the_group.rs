use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::RadioGroupBuilder;

#[test]
fn radio_groups_select_with_space_and_arrows_without_leaving_the_group() {
    let mut document = Document::new();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let group = with_reactive_scope(&mut document, || {
        view! {
            <radio_group labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string()]} selected={None} on_change={Box::new(move |_document: &mut Document, value| {
                sink.borrow_mut().push(value)
            })} />
        }
    });
    let after = labelled_button(&mut document, "After");
    let after_focus = focus_flag(&mut document, after);
    toolbar(&mut document, &[group, after]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        None
    );
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    assert_eq!(
        styled::radio_group_selected(harness.document(), group),
        Some(2)
    );
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(*changes.borrow(), vec![Some(0), Some(2), Some(0)]);
    harness.key(Key::Tab, Modifiers::NONE);
    assert!(after_focus.get());
}
