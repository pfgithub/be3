use super::*;
use crate::reactive::{create_signal, view, with_reactive_scope};
use crate::styled::TextInputBuilder;

#[test]
fn setting_the_value_of_a_text_input_reports_the_change() {
    let reported = Rc::new(RefCell::new(String::new()));
    let sink = reported.clone();
    let (value, set_value) = create_signal(String::new());
    let (document, [_input]) = toolbar_of(|| {
        [view! {
            <text_input value={value} on_change={move |value| {
                *sink.borrow_mut() = value;
            }} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), || {
        set_value.set("typed for you".to_string())
    });

    assert_eq!(reported.borrow().as_str(), "typed for you");
}
