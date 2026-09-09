use super::*;
use crate::reactive::{self, button, create_memo, create_signal, intrinsic, text};

#[test]
fn a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame() {
    let mut document = Document::new();

    let (count, set_count) = create_signal(0i64);
    let (increment, value) = reactive::enter(&mut document, || {
        let increment = button()
            .children([intrinsic(text().string("+".to_string()).build())])
            .on_click(Box::new(move |_document| {
                set_count.update(|count| *count += 1)
            }))
            .build();
        let value = text()
            .string(create_memo(move || count.get().to_string()))
            .build();
        (increment, value)
    });
    toolbar(&mut document, &[increment, value]);

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), value), "1");
}
