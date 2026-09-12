use super::*;
use crate::reactive::{view, Text};
use crate::styled::ListRow;

#[test]
fn list_rows_and_pressables_activate_from_the_keyboard() {
    let count = Rc::new(Cell::new(0));
    let (rows, presses) = (count.clone(), count.clone());
    let (document, [_row, _pressable]) = toolbar_of(|| {
        [
            view! {
                <ListRow on_click={move || rows.set(rows.get() + 1)}>
                    <Text string="Row" font_size=14.0 color=Color32::WHITE />
                </ListRow>
            },
            view! {
                <unstyled::Pressable on_click={move || presses.set(presses.get() + 1)}>
                    <Text string="Press" font_size=14.0 color=Color32::WHITE />
                </unstyled::Pressable>
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
