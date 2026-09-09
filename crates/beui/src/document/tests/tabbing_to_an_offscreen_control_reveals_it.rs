use super::*;

#[test]
fn tabbing_to_an_offscreen_control_reveals_it() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let mut buttons = Vec::new();
    for index in 0..10 {
        let button = labelled_button(&mut document, &format!("Button {index}"));
        document.append_scroll_item(scroll, button);
        buttons.push(button);
    }
    document.set_root(scroll);
    let mut harness = Harness::sized(document, Vec2::new(300.0, 100.0));
    for _ in 0..6 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    assert!(unstyled::button_focused(harness.document(), buttons[4]).get());
    assert!(harness.document.scroll_offset(scroll) > 0.0);
    let rect = harness.rect(buttons[4]);
    assert!(rect.top() >= 0.0 && rect.bottom() <= 100.0);
    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::SHIFT);
    }
    assert!(unstyled::button_focused(harness.document(), buttons[0]).get());
    assert_eq!(harness.document.scroll_offset(scroll), 0.0);
}
