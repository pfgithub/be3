use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::ContextMenuBuilder;

#[test]
fn arrow_keys_move_a_visible_highlight_through_an_open_context_menu() {
    let mut document = Document::new();
    let region = document.create_sized(Some(120.0), Some(60.0));
    let fill = document.create_fill(Color32::from_gray(80), 4);
    document.set_sized_child(region, fill);
    let items = vec![
        unstyled::MenuItem::new("Copy"),
        unstyled::MenuItem::new("Paste"),
    ];
    let menu = with_reactive_scope(&mut document, || {
        view! { <context_menu region={region} items={items} /> }
    });
    toolbar(&mut document, &[menu]);
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
