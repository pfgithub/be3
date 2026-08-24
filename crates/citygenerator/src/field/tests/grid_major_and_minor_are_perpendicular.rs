use super::*;

#[test]
fn grid_major_and_minor_are_perpendicular() {
    let mut field = TensorField::new(0, NoiseParams::default());
    field.add_grid(Vec2::ZERO, 100.0, 0.0, 0.3);
    let major = field.direction(Vec2::new(10.0, 10.0), Eigenvector::Major);
    let minor = field.direction(Vec2::new(10.0, 10.0), Eigenvector::Minor);
    assert!(major.dot(minor).abs() < 1.0e-5);
}
