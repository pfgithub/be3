use super::*;
use crate::reactive::{build, view, NodeRef, TextBuilder};

#[test]
fn dragging_the_inspector_edge_resizes_the_panel() {
    let text = NodeRef::new();
    let document = build({
        let text = text.clone();
        move || {
            view! {
                <text
                    node_ref=&text
                    string="Hello"
                    font_size=14.0
                    color=Color32::WHITE
                />
            }
        }
    });
    let text = text.get();
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);

    harness.toggle_inspector();
    let width = harness.inspector().width;
    let edge = WIDE_VIEWPORT.x - width;

    harness.drag(pos2(edge, 100.0), pos2(edge - 80.0, 100.0));

    assert_eq!(harness.inspector().width, width + 80.0);
    assert_eq!(
        harness.document.node_rect(text).map(|rect| rect.right()),
        Some(edge - 80.0)
    );
}
