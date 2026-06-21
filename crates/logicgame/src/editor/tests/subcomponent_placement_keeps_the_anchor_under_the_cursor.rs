use super::*;
use logicgame::grid::{ComponentFileRef, ComponentHash, ComponentPort};
use uuid::Uuid;

#[test]
fn subcomponent_placement_keeps_the_anchor_under_the_cursor() {
    let anchor = Point::new(8, 8);
    let kind = ComponentKind::subcomponent(
        ComponentFileRef {
            id: Uuid::from_u128(0),
        },
        ComponentHash::new("0".repeat(64)).unwrap(),
        logicgame::grid::Size::new(7, 11),
        vec![ComponentPort::input(0, scale(4), ComponentSide::Left, 0, 4)],
    )
    .unwrap();

    assert_eq!(
        subcomponent_placement_position(anchor, ComponentOrientation::Up, &kind),
        Some(anchor)
    );
    assert_eq!(
        subcomponent_placement_position(anchor, ComponentOrientation::Right, &kind),
        Some(Point::new(0, 8))
    );
    assert_eq!(
        subcomponent_placement_position(anchor, ComponentOrientation::Down, &kind),
        Some(Point::new(4, 0))
    );
    assert_eq!(
        subcomponent_placement_position(anchor, ComponentOrientation::Left, &kind),
        Some(Point::new(8, 4))
    );
}
