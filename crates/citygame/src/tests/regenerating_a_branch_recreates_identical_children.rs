use super::*;

#[test]
fn regenerating_a_branch_recreates_identical_children() {
    let mut world = demo_world(7);
    let root = world.root_id();
    world.expand(&root);
    let before: Vec<_> = (0..5)
        .map(|index| bounds(&world, &root.child(index)))
        .collect();

    assert_eq!(world.unload_descendants(&root), 5);
    world.expand(&root);
    let after: Vec<_> = (0..5)
        .map(|index| bounds(&world, &root.child(index)))
        .collect();
    assert_eq!(before, after);
}
