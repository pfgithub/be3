use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::CheckboxBuilder;

#[test]
fn clicking_a_checkbox_toggles_it() {
    let mut document = Document::new();
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let checkbox = with_reactive_scope(&mut document, || {
        view! {
            <checkbox label={"Show timings".to_string()} checked={false} on_change={Box::new(move |_document: &mut Document, checked| {
                sink.borrow_mut().push(checked);
            })} />
        }
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
