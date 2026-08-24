use super::*;

#[test]
fn leaves_never_insert_children() {
    let mut world = demo_world(3);
    let mut id = NodeId::root();
    for _ in 0..3 {
        assert!(matches!(world.expand(&id), ExpandResult::Expanded(_)));
        id = id.child(0);
    }
    let count = world.len();
    assert_eq!(world.expand(&id), ExpandResult::Leaf);
    assert_eq!(world.state(&id), Some(NodeState::Leaf));
    assert_eq!(world.len(), count);
    assert_eq!(world.expand(&id), ExpandResult::Leaf);
}
