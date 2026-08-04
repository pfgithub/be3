use super::*;

#[test]
fn wire_projection_uses_the_dominant_axis() {
    assert_eq!(
        projected_wire(Point::new(1, 1), Point::new(2, 1), scale(1)),
        Some(wire((1, 1), (2, 1), 1))
    );
    assert_eq!(
        projected_wire(Point::new(0, 0), Point::new(8, 3), scale(1)),
        Some(wire((0, 0), (8, 0), 1))
    );
    assert_eq!(
        projected_wire(Point::new(8, 3), Point::new(0, 0), scale(1)),
        Some(wire((0, 3), (8, 3), 1))
    );
    assert_eq!(
        projected_wire(Point::new(0, 0), Point::new(2, -9), scale(1)),
        Some(wire((0, -9), (0, 0), 1))
    );
    assert_eq!(
        projected_wire(Point::new(2, -9), Point::new(0, 0), scale(1)),
        Some(wire((2, -9), (2, 0), 1))
    );
}
