use super::super::*;

#[test]
fn bitmap_ignores_pixels_outside_bounds() {
    let bitmap = Bitmap::new(10, 10);

    assert_eq!(bitmap.pixel_index(-1, 0), None);
    assert_eq!(bitmap.pixel_index(0, -1), None);
    assert_eq!(bitmap.pixel_index(10, 0), None);
    assert_eq!(bitmap.pixel_index(0, 10), None);
}
