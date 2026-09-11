use super::*;
use crate::reactive::{view, NodeRef};
use crate::styled::ContextMenuBuilder;

#[test]
fn arrow_keys_move_a_visible_highlight_through_an_open_context_menu() {
    let region = NodeRef::new();
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let (document, [_menu]) = toolbar_of({
        let region = region.clone();
        move || {
            [
                view! { <context_menu region={view! { <menu_region node_ref={&region} /> }} items={items} /> },
            ]
        }
    });
    let region = region.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let pos = harness.center(region);
    harness.frame(vec![Event::PointerMoved(pos)]);
    let opened = harness.frame(vec![Event::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed: true,
        modifiers: Modifiers::NONE,
    }]);

    let highlights = |output: &crate::FrameOutput| {
        output
            .shapes()
            .iter()
            .filter(|shape| {
                matches!(shape, crate::Shape::Rect { color, stroke_width, .. }
                    if *color == styled::theme::ACCENT_SOFT && *stroke_width == 0.0)
            })
            .count()
    };
    assert_eq!(highlights(&opened), 0, "nothing should be highlighted yet");

    let moved = harness.frame(vec![key_event(Key::ArrowDown, true, Modifiers::NONE)]);
    assert_eq!(
        highlights(&moved),
        1,
        "first arrow press should highlight an item"
    );

    let moved_again = harness.frame(vec![key_event(Key::ArrowDown, true, Modifiers::NONE)]);
    assert_eq!(
        highlights(&moved_again),
        1,
        "highlight should move rather than disappear"
    );
}
