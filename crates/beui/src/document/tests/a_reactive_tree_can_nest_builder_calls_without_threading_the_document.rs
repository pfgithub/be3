use super::*;
use crate::reactive::{build, button, column, create_memo, create_signal, intrinsic, row, text};

#[test]
fn a_reactive_tree_can_nest_builder_calls_without_threading_the_document() {
    let value = std::rc::Rc::new(std::cell::Cell::new(None));
    let increment = std::rc::Rc::new(std::cell::Cell::new(None));
    let sink_value = value.clone();
    let sink_increment = increment.clone();

    let document = build(move || {
        let (count, set_count) = create_signal(0i64);
        let value_node = text()
            .string(create_memo(move || count.get().to_string()))
            .build();
        let increment_node = button()
            .children([intrinsic(text().string("+".to_string()).build())])
            .on_click(Box::new(move |_document| {
                set_count.update(|count| *count += 1)
            }))
            .build();
        sink_value.set(Some(value_node));
        sink_increment.set(Some(increment_node));
        column()
            .spacing(8.0)
            .children([intrinsic(
                row()
                    .spacing(8.0)
                    .children([intrinsic(increment_node), intrinsic(value_node)])
                    .build(),
            )])
            .build()
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
