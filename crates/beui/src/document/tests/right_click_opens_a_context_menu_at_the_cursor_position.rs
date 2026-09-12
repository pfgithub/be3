use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenu;

#[test]
fn right_click_opens_a_context_menu_at_the_cursor_position() {
    let region = NodeRef::new();
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || [view! { <ContextMenu items><MenuRegion @node_ref=&region /></ContextMenu> }]
    });
    let region = region.get();
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
    let overlay = unstyled::context_menu_overlay(harness.document(), inner);
    let panel = harness
        .document()
        .overlay_content(overlay)
        .expect("open context menu has content");
    let panel_rect = harness.rect(panel);
    assert!((panel_rect.left() - pos.x).abs() < 0.5);
    assert!((panel_rect.top() - pos.y).abs() < 0.5);
}
