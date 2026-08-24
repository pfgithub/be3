use super::*;

#[test]
fn finds_a_block_in_a_square_graph() {
    let square = vec![
        vec![Vec2::ZERO, Vec2::X],
        vec![Vec2::X, Vec2::ONE],
        vec![Vec2::ONE, Vec2::Y],
        vec![Vec2::Y, Vec2::ZERO],
    ];
    let graph = Graph::from_streamlines(&square, 1.0, true);
    let polygons = graph.polygons(20);
    assert_eq!(polygons.len(), 1);
    assert!((polygon_area(&polygons[0]) - 1.0).abs() < 1.0e-5);
}
