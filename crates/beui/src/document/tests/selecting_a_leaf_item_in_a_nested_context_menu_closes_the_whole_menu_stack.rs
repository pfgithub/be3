use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::ContextMenuBuilder;

#[test]
fn selecting_a_leaf_item_in_a_nested_context_menu_closes_the_whole_menu_stack() {
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
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let menu = with_reactive_scope(&mut document, || {
        view! { <context_menu region={region} items={items} on_select={Box::new(move |_document: &mut Document, path| {
            sink.borrow_mut().push(path);
        })} /> }
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
    let overlay = unstyled::context_menu_overlay(harness.document(), inner);

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(selected.borrow().as_slice(), &[vec![0usize, 1usize]]);
    assert!(!harness.document().is_overlay_open(overlay));
}
