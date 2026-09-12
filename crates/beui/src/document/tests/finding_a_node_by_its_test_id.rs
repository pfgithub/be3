use super::*;
use crate::reactive::view;

#[test]
fn finding_a_node_by_its_test_id() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                @test_id={"toolbar.button"}
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(harness.find("toolbar.button")));

    assert_eq!(clicks.get(), 1);
    assert_eq!(harness.document().find_test_id("toolbar.missing"), None);
}
