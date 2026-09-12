use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenuBuilder;

#[test]
fn tab_is_trapped_inside_an_open_context_menu() {
    let region = NodeRef::new();
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let (document, [before, menu, after]) = toolbar_of({
        let region = region.clone();
        move || {
            [
                view! { <labelled_button label={"Before".to_string()} /> },
                view! {
                    <context_menu items={items}>
                        <menu_region node_ref={&region} />
                    </context_menu>
                },
                view! { <labelled_button label={"After".to_string()} /> },
            ]
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
    let content = unstyled::context_menu_menu(harness.document(), inner);
    let root_focusable = unstyled::menu_list_root_focusable(harness.document(), content);

    assert_eq!(harness.document().focused_node(), Some(root_focusable));

    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
        assert_eq!(harness.document().focused_node(), Some(root_focusable));
        assert!(!harness.document().focus_is_within(before));
        assert!(!harness.document().focus_is_within(after));
    }
}
