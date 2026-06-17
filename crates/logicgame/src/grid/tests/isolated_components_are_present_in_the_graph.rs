use super::*;

#[test]
fn isolated_components_are_present_in_the_graph() {
    let mut grid = LogicGrid::new();
    let led = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
    assert_eq!(
        grid.generate_graph().nodes,
        vec![GraphNode::Component { component: led }]
    );
}
