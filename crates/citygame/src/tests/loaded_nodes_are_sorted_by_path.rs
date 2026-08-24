use super::*;

#[test]
fn loaded_nodes_are_sorted_by_path() {
    let mut world = demo_world(13);
    let root = NodeId::root();
    world.expand(&root);
    world.expand(&root.child(2));
    world.expand(&root.child(0));
    let ids: Vec<_> = world.loaded_ids().cloned().collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
}
