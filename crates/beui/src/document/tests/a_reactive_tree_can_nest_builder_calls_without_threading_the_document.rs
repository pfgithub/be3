use super::*;
use crate::reactive::{build, create_memo, create_signal, view, Button, Column, Row, Text};

#[test]
fn a_reactive_tree_can_nest_builder_calls_without_threading_the_document() {
    let value = std::rc::Rc::new(std::cell::Cell::new(None));
    let increment = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_value = value.clone();
    let sink_increment = increment.clone();

    let document = build(move || {
        let (count, set_count) = create_signal(0i64);
        let value_node = view! {
            <Text string={create_memo(move || count.get().to_string())} />
        };
        let increment_node = view! {
            <Button on_click={move || {
                set_count.update(|count| *count += 1)
            }}>
                <Text string="+" />
            </Button>
        };
        sink_value.set(Some(value_node));
        sink_increment.set(Some(increment_node));
        view! {
            <Column spacing=8.0>
                <Row spacing=8.0>
                    {increment_node}
                    {value_node}
                </Row>
            </Column>
        }
    });

    let value = value.get().expect("text node was created");
    let increment = increment.get().expect("button node was created");

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    assert_eq!(text_of(harness.document(), value), "0");

    harness.click(harness.center(increment));
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), value), "1");
}
