use super::*;
use crate::reactive::{create_memo, create_signal, view, Button, Text};

#[test]
fn a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame() {
    let (document, [increment, value]) = toolbar_of(|| {
        let (count, set_count) = create_signal(0i64);
        [
            view! {
                <Button on_click={move || set_count.update(|count| *count += 1)}>
                    <Text string="+" />
                </Button>
            },
            view! { <Text string={create_memo(move || count.get().to_string())} /> },
        ]
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), value), "1");
}
