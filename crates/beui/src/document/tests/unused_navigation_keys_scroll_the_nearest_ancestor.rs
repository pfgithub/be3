use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SliderBuilder;

#[test]
fn unused_navigation_keys_scroll_the_nearest_ancestor() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let tabs = styled::tabs(&mut document, &["One", "Two"], 0);
    document.append_scroll_item(scroll, tabs);
    let slider = with_reactive_scope(&mut document, || view! { <slider value={0.5} /> });
    document.append_scroll_item(scroll, slider);
    for _ in 0..20 {
        let text = document.create_text("Content", 14.0, Color32::WHITE);
        document.append_scroll_item(scroll, text);
    }
    document.set_root(scroll);
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(styled::tabs_selected(harness.document(), tabs), 0);
    assert_eq!(harness.document.scroll_offset(scroll), 40.0);
    harness.key(Key::PageUp, Modifiers::NONE);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert!(styled::slider_value(harness.document(), slider) < 0.5);
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
