use super::*;
use crate::reactive::{create_signal, view, with_reactive_scope, TextBuilder, VisibilityBuilder};

#[test]
fn evicting_a_virtual_scroll_row_disposes_its_effects() {
    let mut document = Document::new();
    let scroll = document.create_scroll();
    let (shown, set_shown) = with_reactive_scope(&mut document, || create_signal(true));

    document.set_scroll_virtual_items(scroll, 100, 20.0, move |index| {
        let shown = shown.clone();
        view! {
            <visibility visible={shown}>
                <text string={format!("Row {index}")} />
            </visibility>
        }
    });
    document.set_root(scroll);

    let mut harness = Harness::sized(document, Vec2::new(400.0, 300.0));
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    *harness.viewport_mut() = Vec2::new(400.0, 0.0);
    harness.frame(Vec::new());
    assert!(harness.document().children(scroll).is_empty());
    *harness.viewport_mut() = Vec2::new(400.0, 300.0);
    harness.frame(Vec::new());
    assert!(!harness.document().children(scroll).is_empty());

    with_installed(harness.document_mut(), |_| set_shown.set(false));
    harness.frame(Vec::new());
}
