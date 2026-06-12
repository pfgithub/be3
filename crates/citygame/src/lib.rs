use glam::{Quat, Vec2, Vec3};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrientedBounds {
    pub center: Vec3,
    pub half_extents: Vec3,
    pub rotation: Quat,
}

impl OrientedBounds {
    pub const fn new(center: Vec3, half_extents: Vec3, rotation: Quat) -> Self {
        Self {
            center,
            half_extents,
            rotation,
        }
    }
}

pub trait Element: fmt::Debug {
    fn bounds(&self) -> OrientedBounds;
    fn generate_children(&self, rng: &mut dyn RngCore) -> GeneratedChildren;
}

#[derive(Debug)]
pub enum GeneratedChildren {
    Leaf,
    Children(Vec<Box<dyn Element>>),
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NodeId(Vec<u32>);

impl NodeId {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn from_path(path: impl Into<Vec<u32>>) -> Self {
        Self(path.into())
    }

    pub fn path(&self) -> &[u32] {
        &self.0
    }

    pub fn child(&self, index: u32) -> Self {
        let mut path = self.0.clone();
        path.push(index);
        Self(path)
    }

    fn is_descendant_of(&self, ancestor: &Self) -> bool {
        self.0.len() > ancestor.0.len() && self.0.starts_with(&ancestor.0)
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return formatter.write_str("root");
        }
        formatter.write_str("root")?;
        for index in &self.0 {
            write!(formatter, "/{index}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeState {
    Unexpanded,
    Expanded,
    Leaf,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExpandResult {
    Expanded(usize),
    AlreadyExpanded,
    Leaf,
    Missing,
}

#[derive(Debug)]
struct Node {
    element: Box<dyn Element>,
    state: NodeState,
}

#[derive(Debug)]
pub struct World {
    seed: u64,
    nodes: BTreeMap<NodeId, Node>,
}

impl World {
    pub fn new(seed: u64, root: Box<dyn Element>) -> Self {
        let mut nodes = BTreeMap::new();
        nodes.insert(
            NodeId::root(),
            Node {
                element: root,
                state: NodeState::Unexpanded,
            },
        );
        Self { seed, nodes }
    }

    pub fn root_id(&self) -> NodeId {
        NodeId::root()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn contains(&self, id: &NodeId) -> bool {
        self.nodes.contains_key(id)
    }

    pub fn element(&self, id: &NodeId) -> Option<&dyn Element> {
        self.nodes.get(id).map(|node| node.element.as_ref())
    }

    pub fn state(&self, id: &NodeId) -> Option<NodeState> {
        self.nodes.get(id).map(|node| node.state)
    }

    pub fn loaded_ids(&self) -> impl DoubleEndedIterator<Item = &NodeId> {
        self.nodes.keys()
    }

    pub fn loaded_nodes(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&NodeId, &dyn Element, NodeState)> {
        self.nodes
            .iter()
            .map(|(id, node)| (id, node.element.as_ref(), node.state))
    }

    pub fn expand(&mut self, id: &NodeId) -> ExpandResult {
        let Some(node) = self.nodes.get(id) else {
            return ExpandResult::Missing;
        };
        match node.state {
            NodeState::Expanded => return ExpandResult::AlreadyExpanded,
            NodeState::Leaf => return ExpandResult::Leaf,
            NodeState::Unexpanded => {}
        }

        let mut rng = ChaCha8Rng::seed_from_u64(seed_for_path(self.seed, id.path()));
        let generated = node.element.generate_children(&mut rng);
        match generated {
            GeneratedChildren::Leaf => {
                self.nodes.get_mut(id).expect("node was just present").state = NodeState::Leaf;
                ExpandResult::Leaf
            }
            GeneratedChildren::Children(children) => {
                let child_count = children.len();
                for (index, element) in children.into_iter().enumerate() {
                    let child_id = id.child(
                        u32::try_from(index)
                            .expect("a node cannot have more than u32::MAX children"),
                    );
                    self.nodes.insert(
                        child_id,
                        Node {
                            element,
                            state: NodeState::Unexpanded,
                        },
                    );
                }
                self.nodes.get_mut(id).expect("node was just present").state = NodeState::Expanded;
                ExpandResult::Expanded(child_count)
            }
        }
    }

    pub fn unload_descendants(&mut self, id: &NodeId) -> usize {
        if !self.nodes.contains_key(id) {
            return 0;
        }
        let descendants: Vec<_> = self
            .nodes
            .keys()
            .filter(|candidate| candidate.is_descendant_of(id))
            .cloned()
            .collect();
        for descendant in &descendants {
            self.nodes.remove(descendant);
        }
        self.nodes.get_mut(id).expect("node was retained").state = NodeState::Unexpanded;
        descendants.len()
    }
}

fn seed_for_path(world_seed: u64, path: &[u32]) -> u64 {
    let mut hash = world_seed ^ 0x9e37_79b9_7f4a_7c15;
    for &index in path {
        hash ^= u64::from(index).wrapping_add(0x9e37_79b9_7f4a_7c15);
        hash = hash.rotate_left(27).wrapping_mul(0x3c79_ac49_2ba7_b653);
        hash ^= hash >> 33;
    }
    hash
}

pub mod city {
    use super::{OrientedBounds, Quat, Vec2, Vec3};
    use rand::{RngCore, SeedableRng};
    use rand_chacha::ChaCha8Rng;

    pub const CITY_HALF_EXTENT_M: f32 = 500.0;
    const GRID_CELLS: usize = 9;
    const MIN_FRONTAGE_M: f32 = 12.0;
    const MAX_FRONTAGE_M: f32 = 30.0;

    #[derive(Clone, Debug, PartialEq)]
    pub struct Road {
        pub centerline: Vec<Vec2>,
        pub width_m: f32,
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct Building {
        /// Canonical footprint. The final point always equals the first point.
        pub footprint: Vec<Vec2>,
        pub height_m: f32,
        pub access_road: usize,
    }

    impl Building {
        pub fn bounds(&self) -> OrientedBounds {
            let points = &self.footprint[..self.footprint.len() - 1];
            let mut longest = Vec2::X;
            let mut longest_squared = 0.0;
            for edge in points.windows(2) {
                let delta = edge[1] - edge[0];
                if delta.length_squared() > longest_squared {
                    longest = delta.normalize();
                    longest_squared = delta.length_squared();
                }
            }
            let side = Vec2::new(-longest.y, longest.x);
            let mut min = Vec2::splat(f32::INFINITY);
            let mut max = Vec2::splat(f32::NEG_INFINITY);
            for point in points {
                let local = Vec2::new(point.dot(longest), point.dot(side));
                min = min.min(local);
                max = max.max(local);
            }
            let center_2d = longest * ((min.x + max.x) * 0.5) + side * ((min.y + max.y) * 0.5);
            OrientedBounds::new(
                Vec3::new(center_2d.x, self.height_m * 0.5, center_2d.y),
                Vec3::new(
                    (max.x - min.x) * 0.5,
                    self.height_m * 0.5,
                    (max.y - min.y) * 0.5,
                ),
                Quat::from_rotation_y(-longest.y.atan2(longest.x)),
            )
        }
    }

    #[derive(Clone, Debug, PartialEq)]
    pub struct CityLayout {
        pub half_extent_m: f32,
        pub roads: Vec<Road>,
        pub buildings: Vec<Building>,
    }

    impl CityLayout {
        pub fn generate(seed: u64) -> Self {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let coordinates = irregular_coordinates(&mut rng);
            let side = GRID_CELLS + 1;
            let mut intersections = vec![Vec2::ZERO; side * side];
            for row in 0..side {
                for column in 0..side {
                    let boundary =
                        row == 0 || column == 0 || row == GRID_CELLS || column == GRID_CELLS;
                    let jitter = if boundary {
                        Vec2::ZERO
                    } else {
                        Vec2::new(
                            random_range(&mut rng, -12.0, 12.0),
                            random_range(&mut rng, -12.0, 12.0),
                        )
                    };
                    intersections[index(column, row)] =
                        Vec2::new(coordinates[column], coordinates[row]) + jitter;
                }
            }

            let mut roads = Vec::with_capacity(side * 2);
            for row in 0..side {
                roads.push(Road {
                    centerline: (0..side)
                        .map(|column| intersections[index(column, row)])
                        .collect(),
                    width_m: road_width(&mut rng, row),
                });
            }
            for column in 0..side {
                roads.push(Road {
                    centerline: (0..side)
                        .map(|row| intersections[index(column, row)])
                        .collect(),
                    width_m: road_width(&mut rng, column),
                });
            }

            let mut buildings = Vec::new();
            for row in 0..GRID_CELLS {
                for column in 0..GRID_CELLS {
                    let corners = [
                        intersections[index(column, row)],
                        intersections[index(column + 1, row)],
                        intersections[index(column + 1, row + 1)],
                        intersections[index(column, row + 1)],
                    ];
                    // Opposite frontages leave a useful open center and avoid lot overlap.
                    add_frontage_buildings(
                        &mut buildings,
                        &mut rng,
                        corners[0],
                        corners[1],
                        polygon_centroid(&corners),
                        row,
                        roads[row].width_m,
                    );
                    add_frontage_buildings(
                        &mut buildings,
                        &mut rng,
                        corners[2],
                        corners[3],
                        polygon_centroid(&corners),
                        row + 1,
                        roads[row + 1].width_m,
                    );
                }
            }

            Self {
                half_extent_m: CITY_HALF_EXTENT_M,
                roads,
                buildings,
            }
        }
    }

    fn irregular_coordinates(rng: &mut dyn RngCore) -> Vec<f32> {
        let weights: Vec<_> = (0..GRID_CELLS)
            .map(|_| random_range(rng, 0.75, 1.25))
            .collect();
        let scale = CITY_HALF_EXTENT_M * 2.0 / weights.iter().sum::<f32>();
        let mut coordinates = Vec::with_capacity(GRID_CELLS + 1);
        coordinates.push(-CITY_HALF_EXTENT_M);
        for weight in weights {
            coordinates.push(coordinates.last().copied().unwrap() + weight * scale);
        }
        *coordinates.last_mut().unwrap() = CITY_HALF_EXTENT_M;
        coordinates
    }

    fn road_width(rng: &mut dyn RngCore, grid_index: usize) -> f32 {
        if grid_index % 4 == 0 {
            random_range(rng, 18.0, 24.0)
        } else {
            random_range(rng, 8.0, 12.0)
        }
    }

    fn add_frontage_buildings(
        buildings: &mut Vec<Building>,
        rng: &mut dyn RngCore,
        start: Vec2,
        end: Vec2,
        block_center: Vec2,
        access_road: usize,
        road_width: f32,
    ) {
        let edge = end - start;
        let length = edge.length();
        let tangent = edge / length;
        let mut inward = Vec2::new(-tangent.y, tangent.x);
        if inward.dot(block_center - (start + end) * 0.5) < 0.0 {
            inward = -inward;
        }
        let corner_clearance = 12.0;
        let usable = length - corner_clearance * 2.0;
        if usable < MIN_FRONTAGE_M {
            return;
        }
        let target = random_range(rng, 18.0, 25.0);
        let count = ((usable / target).round() as usize).max(1);
        let frontage = usable / count as f32;
        if !(MIN_FRONTAGE_M..=MAX_FRONTAGE_M).contains(&frontage) {
            return;
        }

        for lot in 0..count {
            let lot_start = corner_clearance + lot as f32 * frontage;
            let side_setback = random_range(rng, 1.0, 2.5);
            let front_setback = road_width * 0.5 + random_range(rng, 4.0, 8.0);
            let lot_depth = random_range(rng, 20.0, 38.0);
            let building_depth = lot_depth - random_range(rng, 5.0, 9.0);
            let a = start + tangent * (lot_start + side_setback) + inward * front_setback;
            let b =
                start + tangent * (lot_start + frontage - side_setback) + inward * front_setback;
            let rear_skew = random_range(rng, -1.5, 1.5);
            let d = a + inward * building_depth + tangent * rear_skew;
            let c = b + inward * building_depth + tangent * rear_skew;
            buildings.push(Building {
                footprint: vec![a, b, c, d, a],
                height_m: random_range(rng, 7.0, 38.0),
                access_road,
            });
        }
    }

    fn polygon_centroid(points: &[Vec2; 4]) -> Vec2 {
        points.iter().copied().sum::<Vec2>() / points.len() as f32
    }

    fn index(column: usize, row: usize) -> usize {
        row * (GRID_CELLS + 1) + column
    }

    fn random_range(rng: &mut dyn RngCore, min: f32, max: f32) -> f32 {
        let unit = rng.next_u32() as f32 / u32::MAX as f32;
        min + (max - min) * unit
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn generation_is_deterministic() {
            assert_eq!(CityLayout::generate(42), CityLayout::generate(42));
            assert_ne!(CityLayout::generate(42), CityLayout::generate(43));
        }

        #[test]
        fn roads_form_a_connected_shared_intersection_grid() {
            let city = CityLayout::generate(7);
            let side = GRID_CELLS + 1;
            assert_eq!(city.roads.len(), side * 2);
            for row in 0..side {
                for column in 0..side {
                    assert_eq!(
                        city.roads[row].centerline[column],
                        city.roads[side + column].centerline[row]
                    );
                }
            }
        }

        #[test]
        fn geometry_has_realistic_dimensions_and_access() {
            let city = CityLayout::generate(99);
            assert!(!city.buildings.is_empty());
            for (index, road) in city.roads.iter().enumerate() {
                let grid_index = index % (GRID_CELLS + 1);
                let expected = if grid_index % 4 == 0 {
                    18.0..=24.0
                } else {
                    8.0..=12.0
                };
                assert!(expected.contains(&road.width_m));
            }
            for building in &city.buildings {
                assert_eq!(building.footprint.first(), building.footprint.last());
                assert_eq!(building.footprint.len(), 5);
                assert!((7.0..=38.0).contains(&building.height_m));
                assert!(building.access_road < city.roads.len());
                assert!(building
                    .footprint
                    .iter()
                    .all(|point| point.x.abs() <= CITY_HALF_EXTENT_M
                        && point.y.abs() <= CITY_HALF_EXTENT_M));
                assert!(signed_area(&building.footprint).abs() > 20.0);
                let road = &city.roads[building.access_road];
                let distance = distance_to_polyline(building.footprint[0], &road.centerline);
                assert!(distance >= road.width_m * 0.5 + 3.9);
                assert!(distance <= road.width_m * 0.5 + 9.0);
            }
        }

        fn signed_area(points: &[Vec2]) -> f32 {
            points
                .windows(2)
                .map(|edge| edge[0].perp_dot(edge[1]))
                .sum::<f32>()
                * 0.5
        }

        fn distance_to_polyline(point: Vec2, line: &[Vec2]) -> f32 {
            line.windows(2)
                .map(|edge| {
                    let segment = edge[1] - edge[0];
                    let t =
                        ((point - edge[0]).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
                    point.distance(edge[0] + segment * t)
                })
                .fold(f32::INFINITY, f32::min)
        }
    }
}

pub mod demo {
    use super::{Element, GeneratedChildren, OrientedBounds};
    use glam::{Quat, Vec3};
    use rand::RngCore;

    const MAX_DEPTH: u32 = 3;

    #[derive(Debug)]
    pub struct DemoElement {
        bounds: OrientedBounds,
        depth: u32,
    }

    impl DemoElement {
        pub fn root() -> Self {
            Self {
                bounds: OrientedBounds::new(Vec3::ZERO, Vec3::new(6.0, 0.35, 6.0), Quat::IDENTITY),
                depth: 0,
            }
        }
    }

    impl Element for DemoElement {
        fn bounds(&self) -> OrientedBounds {
            self.bounds
        }

        fn generate_children(&self, rng: &mut dyn RngCore) -> GeneratedChildren {
            if self.depth >= MAX_DEPTH {
                return GeneratedChildren::Leaf;
            }

            let child_count = if self.depth == 0 { 5 } else { 3 };
            let scale = 0.43_f32.powi(self.depth as i32);
            let mut children: Vec<Box<dyn Element>> = Vec::with_capacity(child_count);
            for index in 0..child_count {
                let angle = index as f32 * std::f32::consts::TAU / child_count as f32
                    + random_range(rng, -0.25, 0.25);
                let radius = if self.depth == 0 { 3.7 } else { 1.6 } * scale;
                let center = self.bounds.center
                    + Vec3::new(
                        angle.cos() * radius,
                        0.65 + self.depth as f32,
                        angle.sin() * radius,
                    );
                let half_extents = Vec3::new(
                    random_range(rng, 0.45, 0.9),
                    random_range(rng, 0.35, 1.0),
                    random_range(rng, 0.45, 0.9),
                ) * scale.max(0.25);
                let rotation = Quat::from_rotation_y(random_range(
                    rng,
                    -std::f32::consts::PI,
                    std::f32::consts::PI,
                ));
                children.push(Box::new(Self {
                    bounds: OrientedBounds::new(center, half_extents, rotation),
                    depth: self.depth + 1,
                }));
            }
            GeneratedChildren::Children(children)
        }
    }

    fn random_range(rng: &mut dyn RngCore, min: f32, max: f32) -> f32 {
        let unit = rng.next_u32() as f32 / u32::MAX as f32;
        min + (max - min) * unit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_world(seed: u64) -> World {
        World::new(seed, Box::new(demo::DemoElement::root()))
    }

    fn bounds(world: &World, id: &NodeId) -> OrientedBounds {
        world.element(id).unwrap().bounds()
    }

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

    #[test]
    fn branch_generation_order_is_irrelevant() {
        let mut first = demo_world(11);
        let mut second = demo_world(11);
        let root = NodeId::root();
        first.expand(&root);
        second.expand(&root);
        let a = root.child(0);
        let b = root.child(1);

        first.expand(&a);
        first.expand(&b);
        second.expand(&b);
        second.expand(&a);

        for id in [a.child(0), a.child(1), b.child(0), b.child(1)] {
            assert_eq!(bounds(&first, &id), bounds(&second, &id));
        }
    }

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
}
