use glam::{Quat, Vec3};
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
        match node.element.generate_children(&mut rng) {
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
                children.push(Box::new(Self {
                    bounds: OrientedBounds::new(
                        center,
                        half_extents,
                        Quat::from_rotation_y(random_range(
                            rng,
                            -std::f32::consts::PI,
                            std::f32::consts::PI,
                        )),
                    ),
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
mod tests;
