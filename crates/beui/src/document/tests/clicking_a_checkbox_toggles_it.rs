use super::*;

#[test]
fn clicking_a_checkbox_toggles_it() {
    let mut document = Document::new();
    let checkbox = styled::checkbox(&mut document, "Show timings", false);
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    styled::set_checkbox_on_change(&mut document, checkbox, move |_document, checked| {
        sink.borrow_mut().push(checked);
    });
    toolbar(&mut document, &[checkbox]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(styled::checkbox_checked(harness.document(), checkbox));
    assert_eq!(*changes.borrow(), [true]);

    harness.click(harness.center(checkbox));
    harness.frame(Vec::new());

    assert!(!styled::checkbox_checked(harness.document(), checkbox));
    assert_eq!(*changes.borrow(), [true, false]);
}
