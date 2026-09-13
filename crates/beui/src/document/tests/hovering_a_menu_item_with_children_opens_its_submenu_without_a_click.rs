use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenu;

#[test]
fn hovering_a_menu_item_with_children_opens_its_submenu_without_a_click() {
    let region = NodeRef::new();
    let items = vec![unstyled::MenuItem::with_children(
        "Share",
        vec![
            unstyled::MenuItem::new("Email"),
            unstyled::MenuItem::new("Link"),
        ],
    )];
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
    let share_button = unstyled::menu_list_row_button(harness.document(), content, 0);
    let submenu = unstyled::menu_list_row_submenu_content(harness.document(), content, 0)
        .expect("share has a submenu");
    let email_button = unstyled::menu_list_row_button(harness.document(), submenu, 0);

    assert!(harness.document().node_rect(email_button).is_none());

    let share_pos = harness.center(share_button);
    harness.frame(vec![Event::PointerMoved(share_pos)]);

    assert!(harness.document().node_rect(email_button).is_some());
    assert!(unstyled::button_focused(harness.document(), email_button).get());
}
