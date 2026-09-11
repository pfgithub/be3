use super::*;
use crate::reactive::view;
use crate::styled::CheckboxBuilder;

#[test]
fn clicking_a_checkbox_toggles_it() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [checkbox]) = toolbar_of(|| {
        [view! {
            <checkbox label={"Show timings".to_string()} checked={false} on_change={move |checked| {
                sink.borrow_mut().push(checked);
            }} />
        }]
    });
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
