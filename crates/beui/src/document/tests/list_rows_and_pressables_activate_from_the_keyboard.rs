use super::*;
use crate::reactive::{view, with_document, with_reactive_scope};
use crate::styled::ListRowBuilder;

#[test]
fn list_rows_and_pressables_activate_from_the_keyboard() {
    let mut document = Document::new();
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    let row = with_reactive_scope(&mut document, || {
        let text = with_document(|document| document.create_text("Row", 14.0, Color32::WHITE));
        view! { <list_row on_click={Box::new(move |_document: &mut Document| sink.set(sink.get() + 1))}>{text}</list_row> }
    });
    let pressable = with_reactive_scope(&mut document, || {
        unstyled::PressableBuilder::default().build()
    });
    let text = document.create_text("Press", 14.0, Color32::WHITE);
    unstyled::set_pressable_child(&mut document, pressable, text);
    let sink = count.clone();
    unstyled::set_pressable_on_click(&mut document, pressable, move |_| sink.set(sink.get() + 1));
    toolbar(&mut document, &[row, pressable]);
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(count.get(), 2);
}
