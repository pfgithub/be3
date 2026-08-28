use super::*;

#[test]
fn a_triangle_is_filled_with_its_corner_colour() {
    let image = crate::render(&triangle([255, 0, 0, 255]), 0).unwrap();

    assert_eq!(image.get_pixel(1, 1).0, [255, 0, 0, 255]);
    assert_eq!(image.get_pixel(7, 7).0, [0, 0, 0, 255]);
}
