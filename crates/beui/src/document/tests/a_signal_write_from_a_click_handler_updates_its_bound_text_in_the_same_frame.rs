use super::*;
use crate::reactive::{button, create_signal, on_click, text};

#[test]
fn a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame() {
    let mut document = Document::new();

    let (count, set_count) = create_signal(0i64);
    let label = text(&mut document, "+");
    let increment = button(
        &mut document,
        label,
        on_click(move || set_count.update(|count| *count += 1)),
    );
    let value = text(&mut document, count);
    toolbar(&mut document, &[increment, value]);

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(harness.document().text(value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(harness.document().text(value), "1");
}
