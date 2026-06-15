use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardBounds {
    pub min: Point,
    pub max: Point,
}

impl BoardBounds {
    pub const fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn as_f32(self) -> [f32; 4] {
        [
            self.min.x as f32,
            self.min.y as f32,
            self.max.x as f32,
            self.max.y as f32,
        ]
    }
}

impl Default for BoardBounds {
    fn default() -> Self {
        Self::new(Point::new(-5, -5), Point::new(5, 5))
    }
}

impl Size {
    pub const fn new(width: i64, height: i64) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Scale(u8);

impl Scale {
    pub const ONE: Self = Self(1);

    pub fn new(value: u8) -> Result<Self, GeometryError> {
        if value <= 64 && value.is_power_of_two() {
            Ok(Self(value))
        } else {
            Err(GeometryError::InvalidScale(value))
        }
    }

    pub const fn get(self) -> i64 {
        self.0 as i64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rotation {
    Up,
    Right,
    Down,
    Left,
}

impl Rotation {
    fn swaps_axes(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeometryError {
    InvalidScale(u8),
    InvalidWire,
    InvalidSubcomponentSize(Size),
    InvalidSubcomponentPort { size: Size, port: ComponentPort },
    TooManySubcomponentPorts(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Wire {
    pub start: Point,
    pub end: Point,
    pub scale: Scale,
}

impl Wire {
    pub fn new(start: Point, end: Point, scale: Scale) -> Result<Self, GeometryError> {
        let (start, end) = if start.y == end.y && start.x != end.x {
            if start.x < end.x {
                (start, end)
            } else {
                (end, start)
            }
        } else if start.x == end.x && start.y != end.y {
            if start.y < end.y {
                (start, end)
            } else {
                (end, start)
            }
        } else {
            return Err(GeometryError::InvalidWire);
        };
        let wire = Self { start, end, scale };
        if wire.length() < scale.get() {
            Err(GeometryError::InvalidWire)
        } else {
            Ok(wire)
        }
    }

    pub fn orientation(self) -> Orientation {
        if self.start.y == self.end.y {
            Orientation::Horizontal
        } else {
            Orientation::Vertical
        }
    }

    pub fn length(self) -> i64 {
        match self.orientation() {
            Orientation::Horizontal => self.end.x - self.start.x,
            Orientation::Vertical => self.end.y - self.start.y,
        }
    }

    fn interval(self) -> (i64, i64) {
        match self.orientation() {
            Orientation::Horizontal => (self.start.x, self.end.x),
            Orientation::Vertical => (self.start.y, self.end.y),
        }
    }

    fn fixed(self) -> i64 {
        match self.orientation() {
            Orientation::Horizontal => self.start.y,
            Orientation::Vertical => self.start.x,
        }
    }

    fn from_parts(
        orientation: Orientation,
        fixed: i64,
        start: i64,
        end: i64,
        scale: Scale,
    ) -> Self {
        match orientation {
            Orientation::Horizontal => Self {
                start: Point::new(start, fixed),
                end: Point::new(end, fixed),
                scale,
            },
            Orientation::Vertical => Self {
                start: Point::new(fixed, start),
                end: Point::new(fixed, end),
                scale,
            },
        }
    }

    fn rect(self) -> Option<Rect> {
        let scale = self.scale.get();
        match self.orientation() {
            Orientation::Horizontal => Some(Rect {
                min: self.start,
                max: Point::new(
                    self.end.x.checked_add(scale)?,
                    self.start.y.checked_add(scale)?,
                ),
            }),
            Orientation::Vertical => Some(Rect {
                min: self.start,
                max: Point::new(
                    self.start.x.checked_add(scale)?,
                    self.end.y.checked_add(scale)?,
                ),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ComponentKind {
    Not {
        scale: Scale,
    },
    Led,
    Storage {
        scale: Scale,
        value: u64,
    },
    Input {
        scale: Scale,
        id: InputId,
    },
    Output {
        scale: Scale,
        id: OutputId,
    },
    Subcomponent {
        size: Size,
        snap: Scale,
        ports: Vec<ComponentPort>,
    },
}

impl ComponentKind {
    pub fn subcomponent(
        size: Size,
        snap: Scale,
        ports: Vec<ComponentPort>,
    ) -> Result<Self, GeometryError> {
        if size.width <= 0 || size.height <= 0 {
            return Err(GeometryError::InvalidSubcomponentSize(size));
        }
        if ports.len() > usize::from(u16::MAX) + 1 {
            return Err(GeometryError::TooManySubcomponentPorts(ports.len()));
        }
        if let Some(port) = ports.iter().copied().find(|port| !port.is_valid_for(size)) {
            return Err(GeometryError::InvalidSubcomponentPort { size, port });
        }
        Ok(Self::Subcomponent { size, snap, ports })
    }

    pub fn snap(&self) -> Scale {
        match self {
            Self::Not { scale }
            | Self::Storage { scale, .. }
            | Self::Input { scale, .. }
            | Self::Output { scale, .. } => *scale,
            Self::Led => Scale::ONE,
            Self::Subcomponent { snap, .. } => *snap,
        }
    }

    fn unrotated_size(&self) -> Option<Size> {
        match self {
            Self::Not { scale }
            | Self::Storage { scale, .. }
            | Self::Input { scale, .. }
            | Self::Output { scale, .. } => {
                let scale = scale.get();
                Some(Size::new(scale, scale.checked_mul(2)?))
            }
            Self::Led => Some(Size::new(1, 2)),
            Self::Subcomponent { size, .. } => Some(*size),
        }
    }

    fn connection_slot_definitions(&self) -> Vec<ConnectionSlotDefinition> {
        let Some(size) = self.unrotated_size() else {
            return Vec::new();
        };

        match self {
            Self::Not { .. } => vec![
                ConnectionSlotDefinition::new(
                    0,
                    ConnectionDirection::Input,
                    ComponentSide::Bottom,
                    0,
                    size.width,
                ),
                ConnectionSlotDefinition::new(
                    1,
                    ConnectionDirection::Output,
                    ComponentSide::Top,
                    0,
                    size.width,
                ),
            ],
            Self::Led => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Input,
                ComponentSide::Bottom,
                0,
                size.width,
            )],
            Self::Storage { .. } => vec![
                ConnectionSlotDefinition::new(
                    0,
                    ConnectionDirection::Input,
                    ComponentSide::Bottom,
                    0,
                    size.width,
                ),
                ConnectionSlotDefinition::new(
                    1,
                    ConnectionDirection::Output,
                    ComponentSide::Top,
                    0,
                    size.width,
                ),
            ],
            Self::Input { .. } => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Output,
                ComponentSide::Bottom,
                0,
                size.width,
            )],
            Self::Output { .. } => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Input,
                ComponentSide::Bottom,
                0,
                size.width,
            )],
            Self::Subcomponent { ports, .. } => ports
                .iter()
                .enumerate()
                .map(|(id, port)| {
                    ConnectionSlotDefinition::new(
                        id as u16,
                        port.direction,
                        port.side,
                        port.start,
                        port.end,
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputId(pub usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Component {
    pub id: ComponentId,
    pub position: Point,
    pub rotation: Rotation,
    pub kind: ComponentKind,
}

impl Component {
    pub fn size(&self) -> Option<Size> {
        let size = self.kind.unrotated_size()?;
        if self.rotation.swaps_axes() {
            Some(Size::new(size.height, size.width))
        } else {
            Some(size)
        }
    }

    fn rect(&self) -> Option<Rect> {
        let size = self.size()?;
        Some(Rect {
            min: self.position,
            max: Point::new(
                self.position.x.checked_add(size.width)?,
                self.position.y.checked_add(size.height)?,
            ),
        })
    }

    pub fn connection_slots(&self) -> Vec<ConnectionSlot> {
        let Some(unrotated_size) = self.kind.unrotated_size() else {
            return Vec::new();
        };

        self.kind
            .connection_slot_definitions()
            .into_iter()
            .filter_map(|slot| {
                slot.rotate_and_translate(unrotated_size, self.rotation, self.position)
            })
            .collect()
    }

    pub fn boundary_lead(&self, bounds: BoardBounds) -> Option<BoundaryLead> {
        if !matches!(
            self.kind,
            ComponentKind::Input { .. } | ComponentKind::Output { .. }
        ) {
            return None;
        }
        let rect = self.rect()?;
        let scale = self.kind.snap().get();
        let (side, corridor, start, end) = match self.rotation {
            Rotation::Up => {
                let x = rect.min.x as f32 + scale as f32 * 0.5;
                (
                    ComponentSide::Top,
                    Rect {
                        min: Point::new(rect.min.x, bounds.min.y),
                        max: Point::new(rect.max.x, rect.min.y),
                    },
                    [x, rect.min.y as f32],
                    [x, bounds.min.y as f32],
                )
            }
            Rotation::Right => {
                let y = rect.min.y as f32 + scale as f32 * 0.5;
                (
                    ComponentSide::Right,
                    Rect {
                        min: Point::new(rect.max.x, rect.min.y),
                        max: Point::new(bounds.max.x, rect.max.y),
                    },
                    [rect.max.x as f32, y],
                    [bounds.max.x as f32, y],
                )
            }
            Rotation::Down => {
                let x = rect.min.x as f32 + scale as f32 * 0.5;
                (
                    ComponentSide::Bottom,
                    Rect {
                        min: Point::new(rect.min.x, rect.max.y),
                        max: Point::new(rect.max.x, bounds.max.y),
                    },
                    [x, rect.max.y as f32],
                    [x, bounds.max.y as f32],
                )
            }
            Rotation::Left => {
                let y = rect.min.y as f32 + scale as f32 * 0.5;
                (
                    ComponentSide::Left,
                    Rect {
                        min: Point::new(bounds.min.x, rect.min.y),
                        max: Point::new(rect.min.x, rect.max.y),
                    },
                    [rect.min.x as f32, y],
                    [bounds.min.x as f32, y],
                )
            }
        };
        Some(BoundaryLead {
            side,
            start,
            end,
            corridor,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoundaryLead {
    pub side: ComponentSide,
    pub start: [f32; 2],
    pub end: [f32; 2],
    corridor: Rect,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ValidationError {
    ComponentOverflow {
        component: ComponentId,
    },
    ComponentOverlap {
        first: ComponentId,
        second: ComponentId,
    },
    ComponentNotSnapped {
        component: ComponentId,
        snap: Scale,
    },
    WireComponentIntersection {
        wire: Wire,
        component: ComponentId,
    },
    WireNotSnapped {
        wire: Wire,
    },
    WireOverflow {
        wire: Wire,
    },
    BoundaryLeadBlocked {
        component: ComponentId,
        blocker: ComponentId,
    },
}

pub struct LogicGrid {
    wires: Vec<Wire>,
    components: BTreeMap<ComponentId, Component>,
    next_component_id: u64,
    next_input_id: usize,
    next_output_id: usize,
    board_bounds: BoardBounds,
}

impl Default for LogicGrid {
    fn default() -> Self {
        Self {
            wires: Vec::new(),
            components: BTreeMap::new(),
            next_component_id: 0,
            next_input_id: 0,
            next_output_id: 0,
            board_bounds: BoardBounds::default(),
        }
    }
}

impl LogicGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wires(&self) -> &[Wire] {
        &self.wires
    }

    pub fn board_bounds(&self) -> BoardBounds {
        self.board_bounds
    }

    pub fn set_board_bounds(&mut self, bounds: BoardBounds) -> bool {
        if bounds.min.x >= bounds.max.x || bounds.min.y >= bounds.max.y {
            return false;
        }
        self.board_bounds = bounds;
        true
    }

    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.components.values()
    }

    pub fn component(&self, id: ComponentId) -> Option<&Component> {
        self.components.get(&id)
    }

    pub fn add_component(
        &mut self,
        position: Point,
        rotation: Rotation,
        mut kind: ComponentKind,
    ) -> ComponentId {
        match &mut kind {
            ComponentKind::Storage { scale, value } => {
                *value &= value_mask(*scale);
            }
            ComponentKind::Input { id, .. } => {
                *id = InputId(self.next_input_id);
                self.next_input_id = self
                    .next_input_id
                    .checked_add(1)
                    .expect("input ID space exhausted");
            }
            ComponentKind::Output { id, .. } => {
                *id = OutputId(self.next_output_id);
                self.next_output_id = self
                    .next_output_id
                    .checked_add(1)
                    .expect("output ID space exhausted");
            }
            _ => {}
        }
        let id = ComponentId(self.next_component_id);
        self.next_component_id = self
            .next_component_id
            .checked_add(1)
            .expect("component ID space exhausted");
        self.components.insert(
            id,
            Component {
                id,
                position,
                rotation,
                kind,
            },
        );
        id
    }

    pub fn remove_component(&mut self, id: ComponentId) -> Option<Component> {
        self.components.remove(&id)
    }

    pub fn set_component_position(&mut self, id: ComponentId, position: Point) -> bool {
        let Some(component) = self.components.get_mut(&id) else {
            return false;
        };
        component.position = position;
        true
    }

    pub fn set_storage_value(&mut self, id: ComponentId, value: u64) -> bool {
        let Some(Component {
            kind:
                ComponentKind::Storage {
                    scale,
                    value: stored,
                },
            ..
        }) = self.components.get_mut(&id)
        else {
            return false;
        };
        *stored = value & value_mask(*scale);
        true
    }

    pub fn toggle_storage_bit(&mut self, id: ComponentId, bit: u32) -> bool {
        let Some(Component {
            kind: ComponentKind::Storage { scale, value },
            ..
        }) = self.components.get_mut(&id)
        else {
            return false;
        };
        if bit >= scale.get() as u32 {
            return false;
        }
        *value ^= 1_u64 << bit;
        true
    }

    pub fn add_wire(&mut self, wire: Wire) -> &[Wire] {
        self.wires.push(wire);
        self.normalize_wires();
        &self.wires
    }

    pub fn remove_wire(&mut self, removal: Wire) -> &[Wire] {
        let orientation = removal.orientation();
        let fixed = removal.fixed();
        let scale = removal.scale;
        let (remove_start, remove_end) = removal.interval();
        let mut result = Vec::with_capacity(self.wires.len() + 1);

        for wire in self.wires.drain(..) {
            if wire.orientation() != orientation || wire.fixed() != fixed || wire.scale != scale {
                result.push(wire);
                continue;
            }
            let (start, end) = wire.interval();
            if remove_end < start || end < remove_start {
                result.push(wire);
                continue;
            }
            if start < remove_start {
                if let Some(remaining_end) = remove_start.checked_sub(scale.get()) {
                    if remaining_end - start >= scale.get() {
                        result.push(Wire::from_parts(
                            orientation,
                            fixed,
                            start,
                            remaining_end.min(end),
                            scale,
                        ));
                    }
                }
            }
            if remove_end < end {
                if let Some(remaining_start) = remove_end.checked_add(scale.get()) {
                    if end - remaining_start >= scale.get() {
                        result.push(Wire::from_parts(
                            orientation,
                            fixed,
                            remaining_start.max(start),
                            end,
                            scale,
                        ));
                    }
                }
            }
        }

        self.wires = result;
        self.normalize_wires();
        &self.wires
    }

    pub fn validate(&self) -> Vec<ValidationError> {
        let mut errors = BTreeSet::new();
        for wire in &self.wires {
            let scale = wire.scale.get();
            if wire.start.x.rem_euclid(scale) != 0
                || wire.start.y.rem_euclid(scale) != 0
                || wire.end.x.rem_euclid(scale) != 0
                || wire.end.y.rem_euclid(scale) != 0
            {
                errors.insert(ValidationError::WireNotSnapped { wire: *wire });
            }
            if wire.rect().is_none() {
                errors.insert(ValidationError::WireOverflow { wire: *wire });
            }
        }

        let components: Vec<_> = self.components.values().collect();
        for component in &components {
            let snap = component.kind.snap();
            if component.position.x.rem_euclid(snap.get()) != 0
                || component.position.y.rem_euclid(snap.get()) != 0
            {
                errors.insert(ValidationError::ComponentNotSnapped {
                    component: component.id,
                    snap,
                });
            }
            if component.rect().is_none() {
                errors.insert(ValidationError::ComponentOverflow {
                    component: component.id,
                });
            }
        }

        for (index, first) in components.iter().enumerate() {
            let Some(first_rect) = first.rect() else {
                continue;
            };
            for second in components.iter().skip(index + 1) {
                if second
                    .rect()
                    .is_some_and(|second_rect| first_rect.overlaps_area(second_rect))
                {
                    errors.insert(ValidationError::ComponentOverlap {
                        first: first.id,
                        second: second.id,
                    });
                }
            }
        }

        for component in &components {
            let Some(lead) = component.boundary_lead(self.board_bounds) else {
                continue;
            };
            if lead.corridor.min.x > lead.corridor.max.x
                || lead.corridor.min.y > lead.corridor.max.y
            {
                continue;
            }
            for blocker in &components {
                if component.id != blocker.id
                    && blocker
                        .rect()
                        .is_some_and(|rect| lead.corridor.overlaps_area(rect))
                {
                    errors.insert(ValidationError::BoundaryLeadBlocked {
                        component: component.id,
                        blocker: blocker.id,
                    });
                }
            }
        }

        for wire in &self.wires {
            let Some(wire_rect) = wire.rect() else {
                continue;
            };
            for component in &components {
                let slots = component.connection_slots();
                if component.rect().is_some_and(|component_rect| {
                    wire_rect.overlaps_area(component_rect)
                        && !wire_component_intersection_is_contact(*wire, component_rect, &slots)
                }) {
                    errors.insert(ValidationError::WireComponentIntersection {
                        wire: *wire,
                        component: component.id,
                    });
                }
            }
        }
        errors.into_iter().collect()
    }

    pub fn generate_graph(&self) -> CircuitGraph {
        let mut sets = DisjointSets::new(self.wires.len());
        for first in 0..self.wires.len() {
            for second in first + 1..self.wires.len() {
                if wires_connect(self.wires[first], self.wires[second]) {
                    sets.union(first, second);
                }
            }
        }

        let mut grouped = BTreeMap::<usize, Vec<Wire>>::new();
        for (index, wire) in self.wires.iter().copied().enumerate() {
            grouped.entry(sets.find(index)).or_default().push(wire);
        }
        for wires in grouped.values_mut() {
            wires.sort();
        }
        let mut nets: Vec<_> = grouped.into_values().collect();
        nets.sort();

        let mut nodes = Vec::new();
        let mut component_nodes = BTreeMap::new();
        for component in self.components.values() {
            let node = GraphNodeId(nodes.len());
            nodes.push(GraphNode::Component {
                component: component.id,
            });
            component_nodes.insert(component.id, node);
        }

        let mut net_nodes = Vec::new();
        for wires in &nets {
            let node = GraphNodeId(nodes.len());
            nodes.push(GraphNode::WireNet {
                wires: wires.clone(),
            });
            net_nodes.push(node);
        }

        let mut contacts = Vec::new();
        for component in self.components.values() {
            let Some(rect) = component.rect() else {
                continue;
            };
            for slot in component.connection_slots() {
                let connected_nets: Vec<_> = nets
                    .iter()
                    .enumerate()
                    .filter_map(|(net_index, wires)| {
                        wires
                            .iter()
                            .any(|wire| {
                                wire_component_contacts(*wire, rect)
                                    .into_iter()
                                    .any(|contact| contact.overlaps(slot))
                            })
                            .then_some(net_index)
                    })
                    .collect();
                if !connected_nets.is_empty() {
                    contacts.push((component.id, slot, connected_nets));
                }
            }
        }
        contacts.sort();

        let mut edges = Vec::new();
        for (component, slot, connected_nets) in contacts {
            let connection = GraphNodeId(nodes.len());
            nodes.push(GraphNode::Connection {
                component,
                slot: slot.id,
                direction: slot.direction,
                side: slot.side,
                start: slot.start,
                end: slot.end,
            });
            edges.push(GraphEdge::new(component_nodes[&component], connection));
            for net_index in connected_nets {
                edges.push(GraphEdge::new(connection, net_nodes[net_index]));
            }
        }
        edges.sort();

        CircuitGraph { nodes, edges }
    }

    fn normalize_wires(&mut self) {
        let original = std::mem::take(&mut self.wires);
        let mut groups = BTreeMap::<(Scale, Orientation, i64), Vec<(i64, i64)>>::new();
        for wire in &original {
            groups
                .entry((wire.scale, wire.orientation(), wire.fixed()))
                .or_default()
                .push(wire.interval());
        }

        let mut merged = Vec::new();
        for ((scale, orientation, fixed), mut intervals) in groups {
            intervals.sort();
            let mut current: Option<(i64, i64)> = None;
            for (start, end) in intervals {
                match current {
                    Some((current_start, current_end)) if start <= current_end => {
                        current = Some((current_start, current_end.max(end)));
                    }
                    Some((current_start, current_end)) => {
                        merged.push(Wire::from_parts(
                            orientation,
                            fixed,
                            current_start,
                            current_end,
                            scale,
                        ));
                        current = Some((start, end));
                    }
                    None => current = Some((start, end)),
                }
            }
            if let Some((start, end)) = current {
                merged.push(Wire::from_parts(orientation, fixed, start, end, scale));
            }
        }
        merged.sort();

        let mut cuts: Vec<BTreeSet<i64>> = vec![BTreeSet::new(); merged.len()];
        for first in 0..original.len() {
            for second in first + 1..original.len() {
                if original[first].scale != original[second].scale
                    || original[first].orientation() == original[second].orientation()
                {
                    continue;
                }
                add_cut_from_endpoint(original[first], original[second], &merged, &mut cuts);
                add_cut_from_endpoint(original[second], original[first], &merged, &mut cuts);
            }
        }

        let mut normalized = Vec::new();
        for (index, wire) in merged.into_iter().enumerate() {
            let (start, end) = wire.interval();
            let mut cursor = start;
            for cut in &cuts[index] {
                if cursor < *cut && *cut < end {
                    normalized.push(Wire::from_parts(
                        wire.orientation(),
                        wire.fixed(),
                        cursor,
                        *cut,
                        wire.scale,
                    ));
                    cursor = *cut;
                }
            }
            normalized.push(Wire::from_parts(
                wire.orientation(),
                wire.fixed(),
                cursor,
                end,
                wire.scale,
            ));
        }
        normalized.sort();
        self.wires = normalized;
    }
}

pub fn value_mask(scale: Scale) -> u64 {
    let bits = scale.get() as u32;
    if bits == u64::BITS {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    min: Point,
    max: Point,
}

impl Rect {
    fn overlaps_area(self, other: Self) -> bool {
        overlaps(self.min.x, self.max.x, other.min.x, other.max.x)
            && overlaps(self.min.y, self.max.y, other.min.y, other.max.y)
    }

    fn intersection(self, other: Self) -> Option<Self> {
        let intersection = Self {
            min: Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            max: Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        };
        (intersection.min.x < intersection.max.x && intersection.min.y < intersection.max.y)
            .then_some(intersection)
    }
}

fn overlaps(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> bool {
    a_start < b_end && b_start < a_end
}

fn overlaps_closed(a_start: i64, a_end: i64, b_start: i64, b_end: i64) -> bool {
    a_start <= b_end && b_start <= a_end
}

fn wires_connect(first: Wire, second: Wire) -> bool {
    if first.scale != second.scale {
        return false;
    }
    match (first.orientation(), second.orientation()) {
        (Orientation::Horizontal, Orientation::Horizontal)
        | (Orientation::Vertical, Orientation::Vertical) => {
            first.fixed() == second.fixed()
                && overlaps_closed(
                    first.interval().0,
                    first.interval().1,
                    second.interval().0,
                    second.interval().1,
                )
        }
        _ => endpoint_touches_wire(first, second) || endpoint_touches_wire(second, first),
    }
}

fn endpoint_touches_wire(endpoint_wire: Wire, other: Wire) -> bool {
    let Some(other_rect) = other.rect() else {
        return false;
    };
    let scale = endpoint_wire.scale.get();
    match endpoint_wire.orientation() {
        Orientation::Horizontal => {
            let y_start = endpoint_wire.start.y;
            let Some(y_end) = y_start.checked_add(scale) else {
                return false;
            };
            [endpoint_wire.start.x, endpoint_wire.end.x]
                .into_iter()
                .any(|x| {
                    other_rect.min.x <= x
                        && x < other_rect.max.x
                        && overlaps(y_start, y_end, other_rect.min.y, other_rect.max.y)
                })
        }
        Orientation::Vertical => {
            let x_start = endpoint_wire.start.x;
            let Some(x_end) = x_start.checked_add(scale) else {
                return false;
            };
            [endpoint_wire.start.y, endpoint_wire.end.y]
                .into_iter()
                .any(|y| {
                    other_rect.min.y <= y
                        && y < other_rect.max.y
                        && overlaps(x_start, x_end, other_rect.min.x, other_rect.max.x)
                })
        }
    }
}

fn add_cut_from_endpoint(
    endpoint_wire: Wire,
    crossed_wire: Wire,
    merged: &[Wire],
    cuts: &mut [BTreeSet<i64>],
) {
    if !endpoint_touches_wire(endpoint_wire, crossed_wire) {
        return;
    }
    let cut = endpoint_wire.fixed();
    for (index, wire) in merged.iter().enumerate() {
        let (start, end) = wire.interval();
        if wire.scale == crossed_wire.scale
            && wire.orientation() == crossed_wire.orientation()
            && wire.fixed() == crossed_wire.fixed()
            && start < cut
            && cut < end
        {
            cuts[index].insert(cut);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectionDirection {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentPort {
    pub direction: ConnectionDirection,
    pub side: ComponentSide,
    pub start: i64,
    pub end: i64,
}

impl ComponentPort {
    pub const fn input(side: ComponentSide, start: i64, end: i64) -> Self {
        Self {
            direction: ConnectionDirection::Input,
            side,
            start,
            end,
        }
    }

    pub const fn output(side: ComponentSide, start: i64, end: i64) -> Self {
        Self {
            direction: ConnectionDirection::Output,
            side,
            start,
            end,
        }
    }

    fn is_valid_for(self, size: Size) -> bool {
        let boundary_length = match self.side {
            ComponentSide::Top | ComponentSide::Bottom => size.width,
            ComponentSide::Right | ComponentSide::Left => size.height,
        };
        0 <= self.start && self.start < self.end && self.end <= boundary_length
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionSlotId(pub u16);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionSlot {
    pub id: ConnectionSlotId,
    pub direction: ConnectionDirection,
    pub side: ComponentSide,
    pub start: i64,
    pub end: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectionSlotDefinition {
    id: ConnectionSlotId,
    direction: ConnectionDirection,
    side: ComponentSide,
    start: i64,
    end: i64,
}

impl ConnectionSlotDefinition {
    const fn new(
        id: u16,
        direction: ConnectionDirection,
        side: ComponentSide,
        start: i64,
        end: i64,
    ) -> Self {
        Self {
            id: ConnectionSlotId(id),
            direction,
            side,
            start,
            end,
        }
    }

    fn rotate_and_translate(
        self,
        size: Size,
        rotation: Rotation,
        position: Point,
    ) -> Option<ConnectionSlot> {
        let (first, second) = match self.side {
            ComponentSide::Top => (Point::new(self.start, 0), Point::new(self.end, 0)),
            ComponentSide::Right => (
                Point::new(size.width, self.start),
                Point::new(size.width, self.end),
            ),
            ComponentSide::Bottom => (
                Point::new(self.start, size.height),
                Point::new(self.end, size.height),
            ),
            ComponentSide::Left => (Point::new(0, self.start), Point::new(0, self.end)),
        };
        let first = rotate_local_point(first, size, rotation)?;
        let second = rotate_local_point(second, size, rotation)?;
        let first = Point::new(
            position.x.checked_add(first.x)?,
            position.y.checked_add(first.y)?,
        );
        let second = Point::new(
            position.x.checked_add(second.x)?,
            position.y.checked_add(second.y)?,
        );

        if first.y == second.y {
            let side = if first.y == position.y {
                ComponentSide::Top
            } else {
                ComponentSide::Bottom
            };
            Some(ConnectionSlot {
                id: self.id,
                direction: self.direction,
                side,
                start: first.x.min(second.x),
                end: first.x.max(second.x),
            })
        } else if first.x == second.x {
            let side = if first.x == position.x {
                ComponentSide::Left
            } else {
                ComponentSide::Right
            };
            Some(ConnectionSlot {
                id: self.id,
                direction: self.direction,
                side,
                start: first.y.min(second.y),
                end: first.y.max(second.y),
            })
        } else {
            None
        }
    }
}

fn rotate_local_point(point: Point, size: Size, rotation: Rotation) -> Option<Point> {
    match rotation {
        Rotation::Up => Some(point),
        Rotation::Right => Some(Point::new(size.height.checked_sub(point.y)?, point.x)),
        Rotation::Down => Some(Point::new(
            size.width.checked_sub(point.x)?,
            size.height.checked_sub(point.y)?,
        )),
        Rotation::Left => Some(Point::new(point.y, size.width.checked_sub(point.x)?)),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Contact {
    side: ComponentSide,
    start: i64,
    end: i64,
}

impl Contact {
    fn overlaps(self, slot: ConnectionSlot) -> bool {
        self.side == slot.side && self.start < slot.end && slot.start < self.end
    }
}

fn wire_endpoint_rect(wire: Wire, endpoint: Point) -> Option<Rect> {
    let scale = wire.scale.get();
    Some(Rect {
        min: endpoint,
        max: Point::new(
            endpoint.x.checked_add(scale)?,
            endpoint.y.checked_add(scale)?,
        ),
    })
}

fn wire_component_intersection_is_contact(
    wire: Wire,
    component: Rect,
    slots: &[ConnectionSlot],
) -> bool {
    let Some(wire_rect) = wire.rect() else {
        return false;
    };
    let Some(intersection) = wire_rect.intersection(component) else {
        return false;
    };
    [wire.start, wire.end].into_iter().any(|endpoint| {
        let Some(endpoint_rect) = wire_endpoint_rect(wire, endpoint) else {
            return false;
        };
        let on_terminal_side = match wire.orientation() {
            Orientation::Horizontal => {
                endpoint_rect.min.x == component.min.x || endpoint_rect.max.x == component.max.x
            }
            Orientation::Vertical => {
                endpoint_rect.min.y == component.min.y || endpoint_rect.max.y == component.max.y
            }
        };
        on_terminal_side
            && endpoint_rect.intersection(component) == Some(intersection)
            && wire_component_contacts(wire, component)
                .into_iter()
                .any(|contact| slots.iter().any(|slot| contact.overlaps(*slot)))
    })
}

fn wire_component_contacts(wire: Wire, component: Rect) -> Vec<Contact> {
    let Some(rect) = wire.rect() else {
        return Vec::new();
    };
    let mut contacts = Vec::new();
    match wire.orientation() {
        Orientation::Horizontal => {
            let Some(end_x) = wire.end.x.checked_add(wire.scale.get()) else {
                return Vec::new();
            };
            for x in [wire.start.x, end_x] {
                let start = rect.min.y.max(component.min.y);
                let end = rect.max.y.min(component.max.y);
                if start < end && x == component.min.x {
                    contacts.push(Contact {
                        side: ComponentSide::Left,
                        start,
                        end,
                    });
                }
                if start < end && x == component.max.x {
                    contacts.push(Contact {
                        side: ComponentSide::Right,
                        start,
                        end,
                    });
                }
            }
            for endpoint in [wire.start, wire.end] {
                let Some(endpoint_rect) = wire_endpoint_rect(wire, endpoint) else {
                    continue;
                };
                let start = endpoint_rect.min.y.max(component.min.y);
                let end = endpoint_rect.max.y.min(component.max.y);
                if start < end && endpoint_rect.min.x == component.min.x {
                    contacts.push(Contact {
                        side: ComponentSide::Left,
                        start,
                        end,
                    });
                }
                if start < end && endpoint_rect.max.x == component.max.x {
                    contacts.push(Contact {
                        side: ComponentSide::Right,
                        start,
                        end,
                    });
                }
            }
        }
        Orientation::Vertical => {
            let Some(end_y) = wire.end.y.checked_add(wire.scale.get()) else {
                return Vec::new();
            };
            for y in [wire.start.y, end_y] {
                let start = rect.min.x.max(component.min.x);
                let end = rect.max.x.min(component.max.x);
                if start < end && y == component.min.y {
                    contacts.push(Contact {
                        side: ComponentSide::Top,
                        start,
                        end,
                    });
                }
                if start < end && y == component.max.y {
                    contacts.push(Contact {
                        side: ComponentSide::Bottom,
                        start,
                        end,
                    });
                }
            }
            for endpoint in [wire.start, wire.end] {
                let Some(endpoint_rect) = wire_endpoint_rect(wire, endpoint) else {
                    continue;
                };
                let start = endpoint_rect.min.x.max(component.min.x);
                let end = endpoint_rect.max.x.min(component.max.x);
                if start < end && endpoint_rect.min.y == component.min.y {
                    contacts.push(Contact {
                        side: ComponentSide::Top,
                        start,
                        end,
                    });
                }
                if start < end && endpoint_rect.max.y == component.max.y {
                    contacts.push(Contact {
                        side: ComponentSide::Bottom,
                        start,
                        end,
                    });
                }
            }
        }
    }
    contacts
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GraphNodeId(pub usize);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraphNode {
    Component {
        component: ComponentId,
    },
    WireNet {
        wires: Vec<Wire>,
    },
    Connection {
        component: ComponentId,
        slot: ConnectionSlotId,
        direction: ConnectionDirection,
        side: ComponentSide,
        start: i64,
        end: i64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphEdge {
    pub first: GraphNodeId,
    pub second: GraphNodeId,
}

impl GraphEdge {
    fn new(first: GraphNodeId, second: GraphNodeId) -> Self {
        if first <= second {
            Self { first, second }
        } else {
            Self {
                first: second,
                second: first,
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CircuitGraph {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

struct DisjointSets {
    parents: Vec<usize>,
}

impl DisjointSets {
    fn new(length: usize) -> Self {
        Self {
            parents: (0..length).collect(),
        }
    }

    fn find(&mut self, item: usize) -> usize {
        if self.parents[item] != item {
            self.parents[item] = self.find(self.parents[item]);
        }
        self.parents[item]
    }

    fn union(&mut self, first: usize, second: usize) {
        let first = self.find(first);
        let second = self.find(second);
        if first != second {
            let (root, child) = if first < second {
                (first, second)
            } else {
                (second, first)
            };
            self.parents[child] = root;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale(value: u8) -> Scale {
        Scale::new(value).unwrap()
    }

    fn wire(start: (i64, i64), end: (i64, i64), scale: u8) -> Wire {
        Wire::new(
            Point::new(start.0, start.1),
            Point::new(end.0, end.1),
            Scale::new(scale).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn validates_scales_wire_shapes_and_rotated_dimensions() {
        assert!(Scale::new(3).is_err());
        assert!(Wire::new(Point::new(0, 0), Point::new(1, 1), Scale::ONE).is_err());
        assert!(Wire::new(Point::new(0, 0), Point::new(1, 0), Scale::ONE).is_ok());
        assert!(Wire::new(Point::new(0, 0), Point::new(2, 0), scale(2)).is_ok());
        assert!(Wire::new(Point::new(0, 0), Point::new(1, 0), scale(2)).is_err());

        let component = Component {
            id: ComponentId(0),
            position: Point::new(0, 0),
            rotation: Rotation::Right,
            kind: ComponentKind::Not { scale: scale(4) },
        };
        assert_eq!(component.size(), Some(Size::new(8, 4)));
    }

    #[test]
    fn reports_snapping_with_negative_coordinates_and_overflow() {
        let mut grid = LogicGrid::new();
        let component = grid.add_component(
            Point::new(-6, 3),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(4),
                value: 0,
            },
        );
        grid.add_wire(wire((i64::MAX - 1, 0), (i64::MAX - 1, 8), 4));

        let errors = grid.validate();
        assert!(errors.contains(&ValidationError::ComponentNotSnapped {
            component,
            snap: scale(4),
        }));
        assert!(errors
            .iter()
            .any(|error| matches!(error, ValidationError::WireOverflow { .. })));
    }

    #[test]
    fn storage_values_are_initialized_and_masked_to_their_scale() {
        let mut grid = LogicGrid::new();
        let narrow = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(4),
                value: 0,
            },
        );
        let wide = grid.add_component(
            Point::new(64, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(64),
                value: 0,
            },
        );

        assert!(matches!(
            &grid.component(narrow).unwrap().kind,
            ComponentKind::Storage { value: 0, .. }
        ));
        assert!(grid.set_storage_value(narrow, u64::MAX));
        assert!(grid.set_storage_value(wide, u64::MAX));
        assert!(matches!(
            &grid.component(narrow).unwrap().kind,
            ComponentKind::Storage { value: 0b1111, .. }
        ));
        assert!(matches!(
            &grid.component(wide).unwrap().kind,
            ComponentKind::Storage {
                value: u64::MAX,
                ..
            }
        ));
    }

    #[test]
    fn storage_bits_toggle_independently_and_reject_out_of_range_bits() {
        let mut grid = LogicGrid::new();
        let storage = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: scale(4),
                value: 0,
            },
        );
        let led = grid.add_component(Point::new(8, 0), Rotation::Up, ComponentKind::Led);

        assert!(grid.toggle_storage_bit(storage, 3));
        assert!(grid.toggle_storage_bit(storage, 0));
        assert!(!grid.toggle_storage_bit(storage, 4));
        assert!(!grid.toggle_storage_bit(led, 0));
        assert!(matches!(
            &grid.component(storage).unwrap().kind,
            ComponentKind::Storage { value: 0b1001, .. }
        ));

        assert!(grid.toggle_storage_bit(storage, 3));
        assert!(matches!(
            &grid.component(storage).unwrap().kind,
            ComponentKind::Storage { value: 0b0001, .. }
        ));
    }

    #[test]
    fn input_and_output_ids_are_separate_monotonic_namespaces() {
        let mut grid = LogicGrid::new();
        let first_input = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(99),
            },
        );
        let first_output = grid.add_component(
            Point::new(2, 0),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId(99),
            },
        );
        grid.remove_component(first_input);
        let second_input = grid.add_component(
            Point::new(4, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(99),
            },
        );

        assert!(matches!(
            grid.component(first_output).unwrap().kind,
            ComponentKind::Output {
                id: OutputId(0),
                ..
            }
        ));
        assert!(matches!(
            grid.component(second_input).unwrap().kind,
            ComponentKind::Input { id: InputId(1), .. }
        ));
    }

    #[test]
    fn boundary_components_rotate_their_inward_slots_and_outward_leads() {
        let bounds = BoardBounds::new(Point::new(-5, -5), Point::new(20, 30));
        let component = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Right,
            kind: ComponentKind::Input {
                scale: scale(2),
                id: InputId(0),
            },
        };

        assert_eq!(component.size(), Some(Size::new(4, 2)));
        assert_eq!(
            component.connection_slots(),
            vec![ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Output,
                side: ComponentSide::Left,
                start: 20,
                end: 22,
            }]
        );
        let lead = component.boundary_lead(bounds).unwrap();
        assert_eq!(lead.side, ComponentSide::Right);
        assert_eq!(lead.start, [14.0, 21.0]);
        assert_eq!(lead.end, [20.0, 21.0]);

        let at_edge = Component {
            position: Point::new(16, 20),
            ..component
        };
        let lead = at_edge.boundary_lead(bounds).unwrap();
        assert_eq!(lead.start, lead.end);
    }

    #[test]
    fn boundary_leads_are_blocked_by_components_but_not_wires() {
        let mut grid = LogicGrid::new();
        grid.set_board_bounds(BoardBounds::new(Point::new(-5, -5), Point::new(5, 5)));
        let input = grid.add_component(
            Point::new(0, 2),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(0),
            },
        );
        let blocker = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        grid.add_wire(wire((-3, 1), (3, 1), 1));

        assert!(grid
            .validate()
            .contains(&ValidationError::BoundaryLeadBlocked {
                component: input,
                blocker,
            }));

        grid.remove_component(blocker);
        assert!(!grid.validate().iter().any(|error| matches!(
            error,
            ValidationError::BoundaryLeadBlocked { component, .. } if *component == input
        )));
    }

    #[test]
    fn merges_collinear_wires_and_subtracts_partial_ranges() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((0, 0), (8, 0), 1));
        grid.add_wire(wire((6, 0), (20, 0), 1));
        assert_eq!(grid.wires(), &[wire((0, 0), (20, 0), 1)]);

        grid.remove_wire(wire((5, 0), (15, 0), 1));
        assert_eq!(
            grid.wires(),
            &[wire((0, 0), (4, 0), 1), wire((16, 0), (20, 0), 1)]
        );
    }

    #[test]
    fn does_not_merge_collinear_wires_whose_endpoints_only_touch() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((1, 1), (1, 2), 1));
        grid.add_wire(wire((1, 3), (1, 4), 1));

        assert_eq!(
            grid.wires(),
            &[wire((1, 1), (1, 2), 1), wire((1, 3), (1, 4), 1)]
        );
    }

    #[test]
    fn preserves_a_split_at_a_perpendicular_endpoint_junction() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((0, 4), (12, 4), 1));
        grid.add_wire(wire((6, 0), (6, 4), 1));

        assert_eq!(
            grid.wires(),
            &[
                wire((0, 4), (6, 4), 1),
                wire((6, 0), (6, 4), 1),
                wire((6, 4), (12, 4), 1),
            ]
        );
        let net_count = grid
            .generate_graph()
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::WireNet { .. }))
            .count();
        assert_eq!(net_count, 1);
    }

    #[test]
    fn interior_crossings_and_mixed_scales_do_not_connect() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((0, 5), (10, 5), 1));
        grid.add_wire(wire((5, 0), (5, 10), 1));
        grid.add_wire(wire((10, 4), (20, 4), 2));

        let net_count = grid
            .generate_graph()
            .nodes
            .iter()
            .filter(|node| matches!(node, GraphNode::WireNet { .. }))
            .count();
        assert_eq!(net_count, 3);
    }

    #[test]
    fn led_has_a_bottom_input_and_rejects_other_contacts() {
        let mut grid = LogicGrid::new();
        let led = grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
        grid.add_wire(wire((10, 2), (10, 4), 1));
        assert!(grid.validate().is_empty());
        assert!(grid.generate_graph().nodes.iter().any(|node| {
            matches!(
                node,
                GraphNode::Connection {
                    component,
                    slot: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Bottom,
                    start: 10,
                    end: 11,
                } if *component == led
            )
        }));

        let mut crossing_grid = LogicGrid::new();
        let crossed_led =
            crossing_grid.add_component(Point::new(10, 0), Rotation::Up, ComponentKind::Led);
        let crossing = wire((8, 0), (10, 0), 1);
        crossing_grid.add_wire(crossing);
        assert!(crossing_grid
            .validate()
            .contains(&ValidationError::WireComponentIntersection {
                wire: crossing,
                component: crossed_led,
            }));
    }

    #[test]
    fn led_rotation_moves_its_input_with_its_short_edge() {
        let component = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Right,
            kind: ComponentKind::Led,
        };

        assert_eq!(component.size(), Some(Size::new(2, 1)));
        assert_eq!(
            component.connection_slots(),
            vec![ConnectionSlot {
                id: ConnectionSlotId(0),
                direction: ConnectionDirection::Input,
                side: ComponentSide::Left,
                start: 20,
                end: 21,
            }]
        );
    }

    #[test]
    fn wire_endpoint_on_not_gate_terminal_is_a_valid_connection() {
        let mut grid = LogicGrid::new();
        let not = grid.add_component(
            Point::new(1, 3),
            Rotation::Down,
            ComponentKind::Not { scale: Scale::ONE },
        );
        let input = wire((1, 1), (1, 3), 1);
        grid.add_wire(input);

        assert!(grid.validate().is_empty());

        let graph = grid.generate_graph();
        assert!(graph.nodes.iter().any(|node| {
            matches!(
                node,
                GraphNode::Connection {
                    component,
                    slot: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Top,
                    start: 1,
                    end: 2,
                } if *component == not
            )
        }));
    }

    #[test]
    fn not_gate_defines_rotated_input_and_output_slots() {
        let component = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Right,
            kind: ComponentKind::Not { scale: scale(2) },
        };

        assert_eq!(
            component.connection_slots(),
            vec![
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Left,
                    start: 20,
                    end: 22,
                },
                ConnectionSlot {
                    id: ConnectionSlotId(1),
                    direction: ConnectionDirection::Output,
                    side: ComponentSide::Right,
                    start: 20,
                    end: 22,
                },
            ]
        );
    }

    #[test]
    fn wire_endpoint_on_not_gate_non_slot_side_is_an_intersection() {
        let mut grid = LogicGrid::new();
        let not = grid.add_component(
            Point::new(4, 4),
            Rotation::Up,
            ComponentKind::Not { scale: scale(2) },
        );
        let wire = wire((2, 4), (4, 4), 1);
        grid.add_wire(wire);

        assert!(grid
            .validate()
            .contains(&ValidationError::WireComponentIntersection {
                wire,
                component: not,
            }));
        assert!(!grid.generate_graph().nodes.iter().any(
            |node| matches!(node, GraphNode::Connection { component, .. } if *component == not)
        ));
    }

    #[test]
    fn wire_crossing_past_a_component_terminal_is_an_intersection() {
        let mut grid = LogicGrid::new();
        let not = grid.add_component(
            Point::new(1, 3),
            Rotation::Down,
            ComponentKind::Not { scale: Scale::ONE },
        );
        let crossing = wire((1, 1), (1, 4), 1);
        grid.add_wire(crossing);

        assert!(grid
            .validate()
            .contains(&ValidationError::WireComponentIntersection {
                wire: crossing,
                component: not,
            }));
    }

    #[test]
    fn retains_component_overlap_errors() {
        let mut grid = LogicGrid::new();
        let first = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        let second = grid.add_component(Point::new(0, 0), Rotation::Down, ComponentKind::Led);
        assert!(grid
            .validate()
            .contains(&ValidationError::ComponentOverlap { first, second }));
    }

    #[test]
    fn graph_collapses_branches_and_keeps_separate_component_contacts() {
        let mut grid = LogicGrid::new();
        let _not = grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Not { scale: scale(2) },
        );
        let storage = grid.add_component(
            Point::new(10, 0),
            Rotation::Right,
            ComponentKind::Storage {
                scale: scale(2),
                value: 0,
            },
        );
        grid.add_wire(wire((2, 0), (9, 0), 1));
        grid.add_wire(wire((5, -5), (5, 0), 1));
        grid.add_wire(wire((5, 0), (5, 2), 1));
        grid.add_wire(wire((2, 2), (9, 2), 1));

        let graph = grid.generate_graph();
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| matches!(node, GraphNode::Component { .. }))
                .count(),
            2
        );
        assert_eq!(
            graph
                .nodes
                .iter()
                .filter(|node| matches!(node, GraphNode::WireNet { .. }))
                .count(),
            1
        );
        let contacts: Vec<_> = graph
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::Connection { component, .. } => Some(*component),
                _ => None,
            })
            .collect();
        assert_eq!(contacts, vec![storage]);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph, grid.generate_graph());
    }

    #[test]
    fn isolated_components_are_present_in_the_graph() {
        let mut grid = LogicGrid::new();
        let led = grid.add_component(Point::new(0, 0), Rotation::Up, ComponentKind::Led);
        assert_eq!(
            grid.generate_graph().nodes,
            vec![GraphNode::Component { component: led }]
        );
    }

    #[test]
    fn very_long_wires_remain_single_entities() {
        let mut grid = LogicGrid::new();
        grid.add_wire(wire((-10_000_000_000_000, 0), (10_000_000_000_000, 0), 1));
        assert_eq!(grid.wires().len(), 1);
        assert_eq!(grid.wires()[0].length(), 20_000_000_000_000);
    }

    #[test]
    fn storage_has_bottom_input_and_top_output() {
        let component = Component {
            id: ComponentId(0),
            position: Point::new(10, 20),
            rotation: Rotation::Up,
            kind: ComponentKind::Storage {
                scale: scale(2),
                value: 0,
            },
        };

        assert_eq!(
            component.connection_slots(),
            vec![
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Bottom,
                    start: 10,
                    end: 12,
                },
                ConnectionSlot {
                    id: ConnectionSlotId(1),
                    direction: ConnectionDirection::Output,
                    side: ComponentSide::Top,
                    start: 10,
                    end: 12,
                },
            ]
        );
    }

    #[test]
    fn subcomponent_ports_are_validated_and_rotate_with_the_component() {
        let size = Size::new(7, 11);
        let ports = vec![
            ComponentPort::input(ComponentSide::Left, 2, 5),
            ComponentPort::output(ComponentSide::Bottom, 3, 7),
        ];
        let kind = ComponentKind::subcomponent(size, scale(2), ports).unwrap();
        let mut grid = LogicGrid::new();
        let id = grid.add_component(Point::new(6, 4), Rotation::Left, kind);
        let component = grid.component(id).unwrap();
        assert_eq!(component.size(), Some(Size::new(11, 7)));
        assert_eq!(
            component.connection_slots(),
            vec![
                ConnectionSlot {
                    id: ConnectionSlotId(0),
                    direction: ConnectionDirection::Input,
                    side: ComponentSide::Bottom,
                    start: 8,
                    end: 11,
                },
                ConnectionSlot {
                    id: ConnectionSlotId(1),
                    direction: ConnectionDirection::Output,
                    side: ComponentSide::Right,
                    start: 4,
                    end: 8,
                },
            ]
        );
        assert!(grid.validate().is_empty());

        let invalid = ComponentPort::input(ComponentSide::Top, 6, 8);
        assert_eq!(
            ComponentKind::subcomponent(size, scale(2), vec![invalid]),
            Err(GeometryError::InvalidSubcomponentPort {
                size,
                port: invalid,
            })
        );
    }
}
