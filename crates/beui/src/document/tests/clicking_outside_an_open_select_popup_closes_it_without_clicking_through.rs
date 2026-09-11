use super::*;
use crate::reactive::{build, view, NodeRef, RowBuilder};
use crate::styled::SelectBuilder;

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
                <row spacing={20.0}>
                    <select node_ref={&select} options={options} selected={Some(0)} />
                    <unstyled::button
                        node_ref={&other}
                        on_click={move || counter.set(counter.get() + 1)}
                    >
                        <button_face label={"Other".to_string()} />
                    </unstyled::button>
                </row>
            }
        }
    });
    let (select, other) = (select.get(), other.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let inner = harness.document().shadow_root(select);
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
