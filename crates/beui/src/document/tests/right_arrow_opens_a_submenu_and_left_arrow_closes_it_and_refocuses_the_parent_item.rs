use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::ContextMenuBuilder;

#[test]
fn right_arrow_opens_a_submenu_and_left_arrow_closes_it_and_refocuses_the_parent_item() {
    let mut document = Document::new();
    let region = document.create_sized(Some(120.0), Some(60.0));
    let fill = document.create_fill(Color32::from_gray(80), 4);
    document.set_sized_child(region, fill);
    let items = vec![unstyled::MenuItem::with_children(
        "Share",
        vec![
            unstyled::MenuItem::new("Email"),
            unstyled::MenuItem::new("Link"),
        ],
    )];
    let menu = with_reactive_scope(&mut document, || {
        view! { <context_menu region={region} items={items} /> }
    });
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
