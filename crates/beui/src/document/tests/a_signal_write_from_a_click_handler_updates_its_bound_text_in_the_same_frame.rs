use super::*;
use crate::reactive::{self, create_memo, create_signal, view, ButtonBuilder, TextBuilder};

#[test]
fn a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame() {
    let mut document = Document::new();

    let (count, set_count) = create_signal(0i64);
    let (increment, value) = reactive::enter(&mut document, || {
        let increment = view! {
            <button on_click={move || {
                set_count.update(|count| *count += 1)
            }}>
                <text string={"+".to_string()} />
            </button>
        };
        let value = view! {
            <text string={create_memo(move || count.get().to_string())} />
        };
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
