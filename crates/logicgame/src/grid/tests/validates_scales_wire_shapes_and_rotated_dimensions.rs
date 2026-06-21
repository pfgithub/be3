use super::*;

#[test]
fn validates_scales_wire_shapes_and_rotated_dimensions() {
    assert!(Scale::new(3).is_err());
    assert!(Wire::new(Point::new(0, 0), Point::new(1, 1), Scale::ONE).is_err());
    assert!(Wire::new(Point::new(0, 0), Point::new(1, 0), Scale::ONE).is_ok());
    assert!(Wire::new(Point::new(0, 0), Point::new(2, 0), scale(2)).is_ok());
    assert!(Wire::new(Point::new(0, 0), Point::new(1, 0), scale(2)).is_err());

    let component = Component {
        id: ComponentId(0),
        position: Point::new(0, 0),
        rotation: Rotation::Right,
        flip: ComponentFlip::default(),
        kind: ComponentKind::Not { scale: scale(4) },
    };
    assert_eq!(component.size(), Some(Size::new(8, 4)));
}
