use super::*;

#[test]
fn setting_the_value_of_a_text_input_reports_the_change() {
    let mut document = Document::new();
    let input = styled::text_input(&mut document, "");
    let reported = Rc::new(RefCell::new(String::new()));
    let sink = reported.clone();
    styled::set_text_input_on_change(&mut document, input, move |_document, value| {
        *sink.borrow_mut() = value;
    });
    toolbar(&mut document, &[input]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    styled::set_text_input_value(harness.document_mut(), input, "typed for you");

    assert_eq!(reported.borrow().as_str(), "typed for you");
}
