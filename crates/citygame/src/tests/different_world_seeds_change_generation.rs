use super::*;

#[test]
fn different_world_seeds_change_generation() {
    let mut first = demo_world(1);
    let mut second = demo_world(2);
    let root = NodeId::root();
    first.expand(&root);
    second.expand(&root);
    assert_ne!(
        bounds(&first, &root.child(0)),
        bounds(&second, &root.child(0))
    );
}
