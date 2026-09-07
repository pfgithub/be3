use super::*;

#[test]
fn tab_is_trapped_inside_an_open_context_menu() {
    let mut document = Document::new();
    let region = document.create_sized(Some(120.0), Some(60.0));
    let fill = document.create_fill(Color32::from_gray(80), 4);
    document.set_sized_child(region, fill);
    let before = labelled_button(&mut document, "Before");
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let menu = styled::context_menu(&mut document, region, items);
    let after = labelled_button(&mut document, "After");
    toolbar(&mut document, &[before, menu, after]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Event::PointerMoved(pos)]);
    harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(menu);
    let content = unstyled::context_menu_menu(harness.document(), inner);
    let copy = unstyled::menu_list_row_button(harness.document(), content, 0);
    let copy_focusable = unstyled::button_focusable(harness.document(), copy);
    let before_focusable = unstyled::button_focusable(harness.document(), before);
    let after_focusable = unstyled::button_focusable(harness.document(), after);

    assert_eq!(harness.document().focused_node(), Some(copy_focusable));

    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        let focused = harness.document().focused_node();
        assert_eq!(focused, Some(copy_focusable));
        assert_ne!(focused, Some(before_focusable));
        assert_ne!(focused, Some(after_focusable));
    }
}
