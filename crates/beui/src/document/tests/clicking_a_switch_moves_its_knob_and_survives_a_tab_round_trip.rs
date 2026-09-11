use super::*;
use crate::reactive::{
    create_memo, create_signal, view, with_reactive_scope, NodeRef, ShowBuilder,
};
use crate::styled::SwitchBuilder;

#[test]
fn clicking_a_switch_moves_its_knob_and_survives_a_tab_round_trip() {
    let mut document = Document::new();
    let (tab, set_tab) = with_reactive_scope(&mut document, || create_signal(0usize));
    let switch_ref = NodeRef::new();

    let panel = with_reactive_scope(&mut document, || {
        let condition = create_memo({
            let tab = tab.clone();
            move || tab.get() == 0
        });
        let switch_ref = switch_ref.clone();
        view! {
            <show
                condition={condition}
                then={move || view! { <switch node_ref={&switch_ref} on={false} /> }}
            />
        }
    });
    toolbar(&mut document, &[panel]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let switch = switch_ref.get();
    let knob = knob_of(harness.document(), switch);
    let off_rect = harness.rect(knob);

    harness.click(harness.center(switch));
    harness.frame(Vec::new());
    let on_rect = harness.rect(knob);
    assert_ne!(off_rect, on_rect, "clicking the switch must move the knob");
    assert_eq!(
        harness.document().node_detail(switch).as_deref(),
        Some("on")
    );

    with_installed(harness.document_mut(), |_| set_tab.set(1));
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| set_tab.set(0));
    harness.frame(Vec::new());

    harness.click(harness.center(switch));
    harness.frame(Vec::new());
    assert_eq!(harness.rect(knob), off_rect, "the knob must move back off");
    assert_eq!(
        harness.document().node_detail(switch).as_deref(),
        Some("off")
    );
}

fn knob_of(document: &Document, switch: NodeId) -> NodeId {
    let mut id = document.shadow_root(switch);
    loop {
        let children = document.children(id);
        if children.len() > 1 {
            return children[1];
        }
        id = children[0];
    }
}
