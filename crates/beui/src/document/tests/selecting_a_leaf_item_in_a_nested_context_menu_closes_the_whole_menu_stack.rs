use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenuBuilder;

#[test]
fn selecting_a_leaf_item_in_a_nested_context_menu_closes_the_whole_menu_stack() {
    let region = NodeRef::new();
    let items = vec![unstyled::MenuItem::with_children(
        "Share",
        vec![
            unstyled::MenuItem::new("Email"),
            unstyled::MenuItem::new("Link"),
        ],
    )];
    let selected = Rc::new(RefCell::new(Vec::new()));
    let sink = selected.clone();
    let (document, [menu]) = toolbar_of({
        let region = region.clone();
        move || {
            [view! { <context_menu items on_select={move |path| {
                sink.borrow_mut().push(path);
            }}><menu_region @node_ref=&region /></context_menu> }]
        }
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
