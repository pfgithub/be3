use super::*;
use crate::reactive::{view, TextBuilder};
use crate::styled::ListRowBuilder;

#[test]
fn list_rows_and_pressables_activate_from_the_keyboard() {
    let count = Rc::new(Cell::new(0));
    let (rows, presses) = (count.clone(), count.clone());
    let (document, [_row, _pressable]) = toolbar_of(|| {
        [
            view! {
                <list_row on_click={move || rows.set(rows.get() + 1)}>
                    <text string={"Row".to_string()} font_size={14.0} color={Color32::WHITE} />
                </list_row>
            },
            view! {
                <unstyled::pressable on_click={move || presses.set(presses.get() + 1)}>
                    <text string={"Press".to_string()} font_size={14.0} color={Color32::WHITE} />
                </unstyled::pressable>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Space, Modifiers::NONE);
    assert_eq!(count.get(), 2);
}
