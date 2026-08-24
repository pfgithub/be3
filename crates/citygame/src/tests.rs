use super::*;

fn demo_world(seed: u64) -> World {
    World::new(seed, Box::new(demo::DemoElement::root()))
}

fn bounds(world: &World, id: &NodeId) -> OrientedBounds {
    world.element(id).unwrap().bounds()
}
mod branch_generation_order_is_irrelevant;
mod different_world_seeds_change_generation;
mod leaves_never_insert_children;
mod loaded_nodes_are_sorted_by_path;
mod regenerating_a_branch_recreates_identical_children;
mod unloading_is_recursive_and_keeps_unrelated_branches;
