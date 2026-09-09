use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::ContextMenuBuilder;

#[test]
fn hovering_a_context_menu_item_moves_keyboard_focus_to_it() {
    let mut document = Document::new();
    let region = document.create_sized(Some(120.0), Some(60.0));
    let fill = document.create_fill(Color32::from_gray(80), 4);
    document.set_sized_child(region, fill);
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
        unstyled::MenuItem::new("Delete"),
    ];
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
    let paste = unstyled::menu_list_row_button(harness.document(), content, 1);
    let delete = unstyled::menu_list_row_button(harness.document(), content, 2);

    let paste_pos = harness.center(paste);
    harness.frame(vec![Event::PointerMoved(paste_pos)]);

    assert!(unstyled::button_focused(harness.document(), paste));

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(unstyled::button_focused(harness.document(), delete));
}
