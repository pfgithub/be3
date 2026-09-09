use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::ListboxBuilder;

#[test]
fn listbox_navigation_reveals_options_inside_a_tall_scroll_item() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let listbox = with_reactive_scope(&mut document, || {
        view! {
            <listbox labels={vec!["One".to_string(), "Two".to_string(), "Three".to_string(), "Four".to_string(), "Five".to_string(), "Six".to_string()]} selected={Some(0)} />
        }
    });
    document.append_scroll_item(scroll, listbox);
    document.set_root(scroll);
    let mut harness = Harness::sized(document, Vec2::new(300.0, 80.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    assert_eq!(
        styled::listbox_selected(harness.document(), listbox),
        Some(5)
    );
    let focused = harness.document.focused_node().unwrap();
    let rect = harness.rect(focused);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 80.0);
    harness.key(Key::Home, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
