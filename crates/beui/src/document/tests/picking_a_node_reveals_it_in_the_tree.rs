use super::*;
use crate::reactive::{build, view, NodeRef, PaddingBuilder, TextBuilder};

#[test]
fn picking_a_node_reveals_it_in_the_tree() {
    let text = NodeRef::new();
    let document = build({
        let text = text.clone();
        move || {
            view! {
                <column spacing={0.0}>
                    <padding horizontal={4.0} vertical={4.0}>
                        <padding horizontal={4.0} vertical={4.0}>
                            <padding horizontal={4.0} vertical={4.0}>
                                <padding horizontal={4.0} vertical={4.0}>
                                    <text
                                        node_ref={&text}
                                        string={"Hello".to_string()}
                                        font_size={14.0}
                                        color={Color32::WHITE}
                                    />
                                </padding>
                            </padding>
                        </padding>
                    </padding>
                </column>
            }
        }
    });
    let text = text.get();
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    assert_eq!(
        harness.tree(),
        ["column", "  padding", "    padding", "      padding"]
    );

    harness.toggle_picking();
    assert!(harness.inspector().state.picking.get());

    let target = harness
        .document
        .node_rect(text)
        .expect("the text was not laid out")
        .center();
    harness.click(target);
    harness.frame(Vec::new());

    assert!(!harness.inspector().state.picking.get());
    assert_eq!(harness.inspector().state.selected.get(), Some(text));
    assert_eq!(
        harness.tree(),
        [
            "column",
            "  padding",
            "    padding",
            "      padding",
            "        padding",
            "          text",
        ]
    );
}
