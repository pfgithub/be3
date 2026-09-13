use super::*;
use crate::reactive::{build, view, NodeRef, Row};
use crate::styled::Select;

#[test]
fn clicking_outside_an_open_select_popup_closes_it_without_clicking_through() {
    let options: Vec<String> = ["Apple", "Banana"]
        .iter()
        .map(|label| (*label).to_owned())
        .collect();
    let other_clicks = Rc::new(Cell::new(0));
    let counter = other_clicks.clone();
    let (select, other) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (select, other) = (select.clone(), other.clone());
        move || {
            view! {
                <Row spacing=20.0>
                    <Select @node_ref=&select options selected=Some(0) />
                    <unstyled::Button
                        @node_ref=&other
                        on_click={move || counter.set(counter.get() + 1)}
                    >
                        <ButtonFace label="Other" />
                    </unstyled::Button>
                </Row>
            }
        }
    });
    let (select, other) = (select.get(), other.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = select;
    let trigger = unstyled::select_trigger(harness.document(), inner);
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    assert!(styled::select_open(harness.document(), select));

    harness.click(harness.center(other));
    harness.frame(Vec::new());

    assert!(!styled::select_open(harness.document(), select));
    assert_eq!(styled::select_selected(harness.document(), select), Some(0));
    assert_eq!(other_clicks.get(), 0);
}
