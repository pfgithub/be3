use super::*;
use crate::reactive::{build, view, Fill, NodeRef};

#[test]
fn a_click_handler_can_mutate_the_tree_in_the_current_frame() {
    let button = NodeRef::new();
    let document = build({
        let button = button.clone();
        move || {
            view! {
                <LabelledButton
                    @node_ref=&button
                    label="replace"
                    on_click={|| with_document(|document| {
                        let replacement = view! { <Fill color=Color32::BLACK radius=0 /> };
                        document.set_root(replacement);
                    })}
                />
            }
        }
    });
    let button = button.get();
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.click(harness.center(button));
    assert_ne!(harness.document.root(), Some(button));
    assert!(harness.document.node_rect(button).is_none());
    assert_eq!(harness.frame(vec![]).shapes().len(), 1);
}
