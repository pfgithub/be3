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
    const RADIAL_CENTER: Vec2 = Vec2::new(-75.0, 35.0);
    const DOWNTOWN_CENTER: Vec2 = Vec2::new(235.0, 105.0);
    const NEIGHBORHOOD_CENTER: Vec2 = Vec2::new(170.0, -245.0);
    const ORGANIC_CENTER: Vec2 = Vec2::new(-260.0, -225.0);

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
            for edge in self.footprint.windows(2) {
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
            let mut roads = Vec::new();
            add_radial_district(&mut roads, &mut rng);
            let downtown_angle = random_range(&mut rng, -0.28, -0.12);
            add_grid_district(
                &mut roads,
                &mut rng,
                DOWNTOWN_CENTER,
                Vec2::new(150.0, 135.0),
                downtown_angle,
                72.0,
                true,
            );
            let neighborhood_angle = random_range(&mut rng, 0.12, 0.3);
            add_grid_district(
                &mut roads,
                &mut rng,
                NEIGHBORHOOD_CENTER,
                Vec2::new(145.0, 105.0),
                neighborhood_angle,
                64.0,
                false,
            );
            add_organic_district(&mut roads, &mut rng);
            add_arterial(
                &mut roads,
                &mut rng,
                RADIAL_CENTER,
                DOWNTOWN_CENTER,
                Vec2::new(55.0, 70.0),
            );
            add_arterial(
                &mut roads,
                &mut rng,
                RADIAL_CENTER,
                ORGANIC_CENTER,
                Vec2::new(-145.0, -55.0),
            );
            add_arterial(
                &mut roads,
                &mut rng,
                RADIAL_CENTER,
                NEIGHBORHOOD_CENTER,
                Vec2::new(35.0, -145.0),
            );
            add_arterial(
                &mut roads,
                &mut rng,
                DOWNTOWN_CENTER,
                NEIGHBORHOOD_CENTER,
                Vec2::new(300.0, -75.0),
            );

            let mut buildings = Vec::new();
            for road_index in 0..roads.len() {
                add_roadside_buildings(&mut buildings, &mut rng, &roads, road_index);
            }
            add_landmark_tower(&mut buildings, &mut rng);

            Self {
                half_extent_m: CITY_HALF_EXTENT_M,
                roads,
                buildings,
            }
        }
    }

    fn add_radial_district(roads: &mut Vec<Road>, rng: &mut dyn RngCore) {
        const RING_POINTS: usize = 32;
        const SPOKES: usize = 9;
        let phase = random_range(rng, 0.0, std::f32::consts::TAU);
        let mut rings = Vec::new();
        for (ring_index, radius) in [58.0, 112.0, 172.0].into_iter().enumerate() {
            let mut centerline = Vec::with_capacity(RING_POINTS + 1);
            for point_index in 0..RING_POINTS {
                let angle = point_index as f32 / RING_POINTS as f32 * std::f32::consts::TAU;
                let variation =
                    1.0 + 0.045 * (angle * 3.0 + phase).sin() + 0.025 * (angle * 5.0 - phase).sin();
                centerline.push(RADIAL_CENTER + Vec2::from_angle(angle) * radius * variation);
            }
            centerline.push(centerline[0]);
            roads.push(Road {
                centerline: centerline.clone(),
                width_m: if ring_index == 1 {
                    random_range(rng, 18.0, 23.0)
                } else {
                    random_range(rng, 10.0, 15.0)
                },
            });
            rings.push(centerline);
        }
        for spoke_index in 0..SPOKES {
            let point_index = spoke_index * RING_POINTS / SPOKES;
            let mut centerline = vec![RADIAL_CENTER];
            centerline.extend(rings.iter().map(|ring| ring[point_index]));
            roads.push(Road {
                centerline,
                width_m: if spoke_index % 3 == 0 {
                    random_range(rng, 17.0, 22.0)
                } else {
                    random_range(rng, 8.0, 13.0)
                },
            });
        }
    }

    fn add_grid_district(
        roads: &mut Vec<Road>,
        rng: &mut dyn RngCore,
        center: Vec2,
        half_size: Vec2,
        angle: f32,
        spacing: f32,
        dense: bool,
    ) {
        let x_axis = Vec2::from_angle(angle);
        let y_axis = Vec2::new(-x_axis.y, x_axis.x);
        let mut offset = -half_size.y;
        while offset <= half_size.y {
            let wobble = random_range(rng, -5.0, 5.0);
            roads.push(Road {
                centerline: vec![
                    center - x_axis * half_size.x + y_axis * (offset + wobble),
                    center + x_axis * half_size.x + y_axis * (offset - wobble),
                ],
                width_m: grid_road_width(rng, offset, dense),
            });
            offset += spacing * random_range(rng, 0.84, 1.16);
        }
        offset = -half_size.x;
        while offset <= half_size.x {
            let wobble = random_range(rng, -5.0, 5.0);
            roads.push(Road {
                centerline: vec![
                    center + x_axis * (offset + wobble) - y_axis * half_size.y,
                    center + x_axis * (offset - wobble) + y_axis * half_size.y,
                ],
                width_m: grid_road_width(rng, offset, dense),
            });
            offset += spacing * random_range(rng, 0.84, 1.16);
        }
    }

    fn grid_road_width(rng: &mut dyn RngCore, offset: f32, dense: bool) -> f32 {
        if offset.abs() < 25.0 {
            random_range(rng, 18.0, 24.0)
        } else if dense {
            random_range(rng, 10.0, 15.0)
        } else {
            random_range(rng, 8.0, 12.0)
        }
    }

    fn add_organic_district(roads: &mut Vec<Road>, rng: &mut dyn RngCore) {
        const BRANCHES: usize = 7;
        for branch in 0..BRANCHES {
            let angle = branch as f32 / BRANCHES as f32 * std::f32::consts::TAU
                + random_range(rng, -0.22, 0.22);
            let direction = Vec2::from_angle(angle);
            let side = Vec2::new(-direction.y, direction.x);
            let length = random_range(rng, 125.0, 210.0);
            let bend = random_range(rng, -42.0, 42.0);
            let centerline = vec![
                ORGANIC_CENTER,
                ORGANIC_CENTER + direction * length * 0.34 + side * bend * 0.25,
                ORGANIC_CENTER + direction * length * 0.68 + side * bend,
                ORGANIC_CENTER + direction * length + side * bend * 0.55,
            ];
            roads.push(Road {
                centerline: centerline.clone(),
                width_m: if branch % 3 == 0 {
                    random_range(rng, 14.0, 19.0)
                } else {
                    random_range(rng, 8.0, 12.0)
                },
            });
            if branch % 2 == 0 {
                let junction = centerline[2];
                let fork_direction = Vec2::from_angle(angle + random_range(rng, 0.55, 0.9));
                roads.push(Road {
                    centerline: vec![
                        junction,
                        junction + fork_direction * random_range(rng, 45.0, 75.0),
                        junction
                            + fork_direction * random_range(rng, 85.0, 120.0)
                            + side * random_range(rng, -18.0, 18.0),
                    ],
                    width_m: random_range(rng, 7.0, 10.0),
                });
            }
        }
        for radius in [62.0, 118.0] {
            let mut centerline = Vec::new();
            for point in 0..20 {
                let angle = point as f32 / 20.0 * std::f32::consts::TAU;
                centerline.push(
                    ORGANIC_CENTER
                        + Vec2::from_angle(angle) * radius * (1.0 + 0.08 * (angle * 3.0).sin()),
                );
            }
            centerline.push(centerline[0]);
            roads.push(Road {
                centerline,
                width_m: random_range(rng, 8.0, 12.0),
            });
        }
    }

    fn add_arterial(
        roads: &mut Vec<Road>,
        rng: &mut dyn RngCore,
        start: Vec2,
        end: Vec2,
        control: Vec2,
    ) {
        let mut centerline = Vec::with_capacity(9);
        for step in 0..=8 {
            let t = step as f32 / 8.0;
            let point =
                start * (1.0 - t).powi(2) + control * (2.0 * (1.0 - t) * t) + end * t.powi(2);
            centerline.push(point);
        }
        roads.push(Road {
            centerline,
            width_m: random_range(rng, 20.0, 27.0),
        });
    }

    fn add_landmark_tower(buildings: &mut [Building], rng: &mut dyn RngCore) {
        let Some(landmark) = buildings
            .iter_mut()
            .filter(|building| urban_density(polygon_center(&building.footprint)) > 0.72)
            .max_by(|a, b| {
                urban_density(polygon_center(&a.footprint))
                    .total_cmp(&urban_density(polygon_center(&b.footprint)))
            })
        else {
            return;
        };
        landmark.height_m = landmark.height_m.max(random_range(rng, 155.0, 235.0));
    }

    fn add_roadside_buildings(
        buildings: &mut Vec<Building>,
        rng: &mut dyn RngCore,
        roads: &[Road],
        road_index: usize,
    ) {
        let road = &roads[road_index];
        for segment in road.centerline.windows(2) {
            let edge = segment[1] - segment[0];
            let length = edge.length();
            if length < 18.0 {
                continue;
            }
            let tangent = edge / length;
            let normal = Vec2::new(-tangent.y, tangent.x);
            for side in [-1.0, 1.0] {
                let mut cursor = random_range(rng, 5.0, 14.0);
                let mut lot_index = 0;
                while cursor < length - 5.0 {
                    let sample = segment[0] + tangent * cursor;
                    let density = urban_density(sample);
                    let (frontage, depth, height) = building_dimensions(rng, density);
                    if cursor + frontage > length - 4.0 {
                        break;
                    }
                    let gap = random_range(rng, 2.0, if density > 0.6 { 5.0 } else { 10.0 });
                    let setback = road.width_m * 0.5
                        + random_range(rng, if density > 0.65 { 3.0 } else { 5.0 }, 10.0);
                    let front_center =
                        segment[0] + tangent * (cursor + frontage * 0.5) + normal * side * setback;
                    let inward = normal * side;
                    let skew = tangent * random_range(rng, -2.0, 2.0);
                    let a = front_center - tangent * frontage * 0.5;
                    let b = front_center + tangent * frontage * 0.5;
                    let c = b + inward * depth + skew;
                    let d = a + inward * depth + skew;
                    let triangular = (road_index + lot_index) % 11 == 0
                        || (density > 0.5 && random_range(rng, 0.0, 1.0) < 0.08);
                    let footprint = if triangular {
                        let apex = (c + d) * 0.5
                            + tangent * random_range(rng, -frontage * 0.18, frontage * 0.18);
                        vec![a, b, apex, a]
                    } else {
                        vec![a, b, c, d, a]
                    };
                    if footprint.iter().all(|point| inside_city(*point))
                        && clear_of_other_roads(&footprint, roads, road_index)
                        && buildings
                            .iter()
                            .all(|other| !footprints_overlap(&footprint, &other.footprint, 1.5))
                    {
                        buildings.push(Building {
                            footprint,
                            height_m: height,
                            access_road: road_index,
                        });
                    }
                    cursor += frontage + gap;
                    lot_index += 1;
                }
            }
        }
    }

    fn urban_density(point: Vec2) -> f32 {
        [
            (RADIAL_CENTER, 145.0, 0.92),
            (DOWNTOWN_CENTER, 125.0, 1.0),
            (NEIGHBORHOOD_CENTER, 115.0, 0.42),
            (ORGANIC_CENTER, 105.0, 0.5),
        ]
        .into_iter()
        .map(|(center, radius, strength)| {
            let distance = point.distance(center) / radius;
            (-distance * distance * 1.7).exp() * strength
        })
        .fold(0.05, f32::max)
    }

    fn building_dimensions(rng: &mut dyn RngCore, density: f32) -> (f32, f32, f32) {
        let roll = random_range(rng, 0.0, 1.0);
        if density > 0.72 && roll < density {
            (
                random_range(rng, 24.0, 48.0),
                random_range(rng, 26.0, 52.0),
                random_range(rng, 75.0, 245.0) * (0.7 + density * 0.45),
            )
        } else if density > 0.32 || roll < density * 0.8 {
            (
                random_range(rng, 16.0, 35.0),
                random_range(rng, 18.0, 38.0),
                random_range(rng, 18.0, 78.0) * (0.75 + density * 0.55),
            )
        } else {
            (
                random_range(rng, 8.0, 19.0),
                random_range(rng, 10.0, 23.0),
                random_range(rng, 5.0, 13.0),
            )
        }
    }

    fn inside_city(point: Vec2) -> bool {
        point.x.abs() <= CITY_HALF_EXTENT_M - 3.0 && point.y.abs() <= CITY_HALF_EXTENT_M - 3.0
    }

    fn clear_of_other_roads(footprint: &[Vec2], roads: &[Road], access_road: usize) -> bool {
        let points = open_polygon(footprint);
        let center = polygon_center(footprint);
        roads.iter().enumerate().all(|(index, road)| {
            index == access_road
                || points
                    .iter()
                    .copied()
                    .chain(std::iter::once(center))
                    .all(|point| {
                        distance_to_polyline(point, &road.centerline) > road.width_m * 0.5 + 2.0
                    })
        })
    }

    fn footprints_overlap(a: &[Vec2], b: &[Vec2], clearance: f32) -> bool {
        axes(a).chain(axes(b)).all(|axis| {
            let (a_min, a_max) = projected_range(a, axis);
            let (b_min, b_max) = projected_range(b, axis);
            a_max + clearance > b_min && b_max + clearance > a_min
        })
    }

    fn axes(points: &[Vec2]) -> impl Iterator<Item = Vec2> + '_ {
        points.windows(2).map(|edge| {
            let direction = edge[1] - edge[0];
            Vec2::new(-direction.y, direction.x).normalize()
        })
    }

    fn projected_range(points: &[Vec2], axis: Vec2) -> (f32, f32) {
        open_polygon(points)
            .iter()
            .map(|point| point.dot(axis))
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            })
    }

    fn open_polygon(points: &[Vec2]) -> &[Vec2] {
        &points[..points.len() - 1]
    }

    fn polygon_center(points: &[Vec2]) -> Vec2 {
        let points = open_polygon(points);
        points.iter().copied().sum::<Vec2>() / points.len() as f32
    }

    fn distance_to_polyline(point: Vec2, line: &[Vec2]) -> f32 {
        line.windows(2)
            .map(|edge| {
                let segment = edge[1] - edge[0];
                let t = ((point - edge[0]).dot(segment) / segment.length_squared()).clamp(0.0, 1.0);
                point.distance(edge[0] + segment * t)
            })
            .fold(f32::INFINITY, f32::min)
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
        fn roads_mix_connected_grid_radial_and_organic_patterns() {
            let city = CityLayout::generate(7);
            assert!(city.roads.len() > 35);
            assert!(
                city.roads
                    .iter()
                    .filter(|road| road.centerline.len() == 2)
                    .count()
                    > 12
            );
            assert!(
                city.roads
                    .iter()
                    .filter(|road| road.centerline.first() == road.centerline.last())
                    .count()
                    >= 5
            );
            assert!(city.roads.iter().any(|road| road.centerline.len() >= 4
                && road.centerline.windows(3).any(|points| {
                    let a = (points[1] - points[0]).normalize();
                    let b = (points[2] - points[1]).normalize();
                    a.dot(b) < 0.995
                })));
            for center in [
                RADIAL_CENTER,
                DOWNTOWN_CENTER,
                NEIGHBORHOOD_CENTER,
                ORGANIC_CENTER,
            ] {
                assert!(city
                    .roads
                    .iter()
                    .any(|road| road.centerline.contains(&center)));
            }
        }

        #[test]
        fn geometry_has_realistic_dimensions_and_access() {
            let city = CityLayout::generate(99);
            assert!(!city.buildings.is_empty());
            for building in &city.buildings {
                assert_eq!(building.footprint.first(), building.footprint.last());
                assert!((4..=5).contains(&building.footprint.len()));
                assert!((5.0..=280.0).contains(&building.height_m));
                assert!(building.access_road < city.roads.len());
                assert!(building
                    .footprint
                    .iter()
                    .all(|point| point.x.abs() <= CITY_HALF_EXTENT_M
                        && point.y.abs() <= CITY_HALF_EXTENT_M));
                assert!(signed_area(&building.footprint).abs() > 20.0);
                let road = &city.roads[building.access_road];
                let distance = distance_to_polyline(building.footprint[0], &road.centerline);
                assert!(distance >= road.width_m * 0.5 + 2.9);
                assert!(distance <= road.width_m * 0.5 + 10.1);
            }
            assert!(city
                .buildings
                .iter()
                .any(|building| building.footprint.len() == 4));
            assert!(city
                .buildings
                .iter()
                .any(|building| building.footprint.len() == 5));
            assert!(city
                .buildings
                .iter()
                .any(|building| building.height_m < 14.0));
            assert!(city
                .buildings
                .iter()
                .any(|building| building.height_m > 100.0));
            let footprints: Vec<_> = city
                .buildings
                .iter()
                .map(|building| building.bounds().half_extents)
                .collect();
            assert!(footprints.iter().any(|bounds| bounds.x.min(bounds.z) < 6.0));
            assert!(footprints
                .iter()
                .any(|bounds| bounds.x.max(bounds.z) > 20.0));

            let tallest = city
                .buildings
                .iter()
                .max_by(|a, b| a.height_m.total_cmp(&b.height_m))
                .unwrap();
            let center = polygon_center(&tallest.footprint);
            assert!(urban_density(center) > 0.7);
        }

        fn signed_area(points: &[Vec2]) -> f32 {
            points
                .windows(2)
                .map(|edge| edge[0].perp_dot(edge[1]))
                .sum::<f32>()
                * 0.5
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
