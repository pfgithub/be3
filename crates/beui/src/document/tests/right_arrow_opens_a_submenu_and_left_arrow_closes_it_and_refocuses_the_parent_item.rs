use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenu;

#[test]
fn right_arrow_opens_a_submenu_and_left_arrow_closes_it_and_refocuses_the_parent_item() {
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

    let inner = harness.document().shadow_root(menu);
    let content = unstyled::context_menu_menu(harness.document(), inner);
    let share_button = unstyled::menu_list_row_button(harness.document(), content, 0);
    let submenu = unstyled::menu_list_row_submenu_content(harness.document(), content, 0)
        .expect("share has a submenu");

    assert!(!unstyled::button_focused(harness.document(), share_button).get());

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::button_focused(harness.document(), share_button).get());

    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());

    let email_button = unstyled::menu_list_row_button(harness.document(), submenu, 0);
    assert!(unstyled::button_focused(harness.document(), email_button).get());
    assert!(harness.document().node_rect(email_button).is_some());

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::button_focused(harness.document(), share_button).get());
    assert!(harness.document().node_rect(email_button).is_none());
}
