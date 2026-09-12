use super::*;
use crate::reactive::view;

#[test]
fn clicking_the_padding_around_a_button_label_activates_it() {
    let clicks = Rc::new(Cell::new(0));
    let counter = clicks.clone();
    let (document, [_button]) = toolbar_of(|| {
        [view! {
            <LabelledButton
                label="Click me"
                on_click={move || counter.set(counter.get() + 1)}
            />
        }]
    });
    let mut harness = Harness::new(document);

    harness.click(pos2(4.0, 4.0));

    assert_eq!(clicks.get(), 1);
}
