use super::*;
use crate::reactive::{build, ScrollBuilder};

#[test]
fn the_scroll_position_is_reported_to_its_listener() {
    let reported = Rc::new(Cell::new(None));
    let sink = reported.clone();
    let rows: Vec<_> = (0..100).map(|index| format!("Row {index}")).collect();
    let document = build(move || {
        let items: Vec<_> = rows
            .into_iter()
            .map(|row| {
                intrinsic(view! {
                    <text string={row} font_size={14.0} color={Color32::WHITE} />
                })
            })
            .collect();
        view! {
            <column spacing={0.0}>
                @percent(100.0) <scroll
                    on_change={move |position| sink.set(Some(position))}
                    children={items}
                />
            </column>
        }
    });
    let mut harness = Harness::new(document);

    harness.frame(Vec::new());

    let position = reported
        .get()
        .expect("the scroll never reported a position");
    assert_eq!(position.offset, 0.0);
    assert_eq!(position.viewport, VIEWPORT.y);
    assert!(position.content > position.viewport);
}
