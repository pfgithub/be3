use std::cell::Cell;
use std::rc::Rc;

use accesskit::{Action, ActionRequest, Role, TreeId};

use super::{build, styled, view, Harness, VIEWPORT};

#[test]
fn accessibility_exposes_and_operates_a_button() {
    let clicks = Rc::new(Cell::new(0));
    let count = clicks.clone();
    let document = build(move || {
        view! {
            <styled::Button
                label="Save"
                variant=styled::ButtonVariant::Primary
                on_click={move || count.set(count.get() + 1)}
            />
        }
    });
    let mut harness = Harness::new(document);

    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);
    let (button_id, button) = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Button)
        .expect("button is absent from the accessibility tree");
    assert!(button.supports_action(Action::Focus));
    assert!(button.supports_action(Action::Click));
    let label = tree
        .nodes
        .iter()
        .find(|(_, node)| node.role() == Role::Label && node.value() == Some("Save"));
    assert!(label.is_some());
    let button_id = *button_id;

    harness.context.accessibility_action(ActionRequest {
        action: Action::Click,
        target_tree: TreeId::ROOT,
        target_node: button_id,
        data: None,
    });
    let output = harness.frame(Vec::new());
    let tree = output.accessibility_tree("Test", VIEWPORT);

    assert_eq!(clicks.get(), 1);
    assert_eq!(tree.focus, button_id);
}
