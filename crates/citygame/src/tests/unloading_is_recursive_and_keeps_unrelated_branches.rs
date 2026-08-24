use super::*;

#[test]
fn unloading_is_recursive_and_keeps_unrelated_branches() {
    let mut world = demo_world(5);
    let root = NodeId::root();
    world.expand(&root);
    let first = root.child(0);
    let second = root.child(1);
    world.expand(&first);
    world.expand(&first.child(0));
    world.expand(&second);

    assert_eq!(world.unload_descendants(&first), 6);
    assert!(world.contains(&first));
    assert!(!world.contains(&first.child(0)));
    assert!(world.contains(&second.child(0)));
    assert_eq!(world.state(&first), Some(NodeState::Unexpanded));
}
