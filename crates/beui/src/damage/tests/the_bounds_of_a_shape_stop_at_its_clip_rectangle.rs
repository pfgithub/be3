use super::*;

#[test]
fn the_bounds_of_a_shape_stop_at_its_clip_rectangle() {
    let shape = clipped(rect(0.0, 0.0, 100.0, 100.0), rect(20.0, 20.0, 60.0, 60.0));

    assert_eq!(bounds(&shape), rect(20.0, 20.0, 60.0, 60.0));
}
