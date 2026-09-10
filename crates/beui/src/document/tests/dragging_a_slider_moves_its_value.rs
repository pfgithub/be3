use super::*;
use crate::reactive::{view, with_reactive_scope};
use crate::styled::SliderBuilder;

#[test]
fn dragging_a_slider_moves_its_value() {
    let mut document = Document::new();
    let reported = Rc::new(Cell::new(0.0));
    let sink = reported.clone();
    let slider = with_reactive_scope(&mut document, || {
        view! { <slider value={0.0} on_change={move |value| sink.set(value)} /> }
    });
    toolbar(&mut document, &[slider]);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let track = harness.rect(slider);
    let middle = track.center();
    harness.drag(pos2(track.left() + 1.0, middle.y), middle);
    harness.frame(Vec::new());

    let value = styled::slider_value(harness.document(), slider);
    assert!((value - 0.5).abs() < 0.01, "the slider read {value}");
    assert_eq!(reported.get(), value);
}
