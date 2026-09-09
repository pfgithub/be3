use super::*;

#[test]
fn list_rows_and_pressables_activate_from_the_keyboard() {
    let mut document = Document::new();
    let text = document.create_text("Row", 14.0, Color32::WHITE);
    let row = styled::list_row(&mut document, text);
    let count = Rc::new(Cell::new(0));
    let sink = count.clone();
    styled::set_list_row_on_click(&mut document, row, move |_| sink.set(sink.get() + 1));
    let pressable = unstyled::pressable(&mut document);
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
