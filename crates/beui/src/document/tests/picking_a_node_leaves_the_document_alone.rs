use super::*;
use crate::reactive::view;

#[test]
fn picking_a_node_leaves_the_document_alone() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <labelled_button
                label="Click me"
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();
    harness.toggle_picking();
    harness.click(pos2(4.0, 4.0));
    harness.frame(Vec::new());

    assert_eq!(clicks.get(), 0);
    assert!(harness.inspector().state.selected.get().is_some());
}
