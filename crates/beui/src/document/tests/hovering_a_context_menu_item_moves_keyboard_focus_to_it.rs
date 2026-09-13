use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenu;

#[test]
fn hovering_a_context_menu_item_moves_keyboard_focus_to_it() {
    let region = NodeRef::new();
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
        unstyled::MenuItem::new("Delete"),
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

    let inner = menu;
    let content = unstyled::context_menu_menu(harness.document(), inner);
    let paste = unstyled::menu_list_row_button(harness.document(), content, 1);
    let delete = unstyled::menu_list_row_button(harness.document(), content, 2);

    let paste_pos = harness.center(paste);
    harness.frame(vec![Event::PointerMoved(paste_pos)]);

    assert!(unstyled::button_focused(harness.document(), paste).get());

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::button_focused(harness.document(), delete).get());
}
