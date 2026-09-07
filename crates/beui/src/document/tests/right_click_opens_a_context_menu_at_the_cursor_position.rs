use super::*;

#[test]
fn right_click_opens_a_context_menu_at_the_cursor_position() {
    let mut document = Document::new();
    let region = document.create_sized(Some(120.0), Some(60.0));
    let fill = document.create_fill(Color32::from_gray(80), 4);
    document.set_sized_child(region, fill);
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let menu = styled::context_menu(&mut document, region, items);
    toolbar(&mut document, &[menu]);
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
    assert_eq!(unstyled::menu_list_len(harness.document(), content), 2);
    let content_rect = harness.rect(content);
    assert!((content_rect.left() - pos.x).abs() < 0.5);
    assert!((content_rect.top() - pos.y).abs() < 0.5);
}
