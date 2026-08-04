use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Point {
    pub x: i64,
    pub y: i64,
}

impl Point {
    pub const fn new(x: i64, y: i64) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Size {
    pub width: i64,
    pub height: i64,
}

impl Size {
    pub const fn new(width: i64, height: i64) -> Self {
        Self { width, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Rotation {
    Up,
    Right,
    Down,
    Left,
}

impl Rotation {
    const fn swaps_axes(self) -> bool {
        matches!(self, Self::Right | Self::Left)
    }
    pub fn flip(self) -> Rotation {
        match self {
            Rotation::Up => Rotation::Down,
            Rotation::Right => Rotation::Left,
            Rotation::Down => Rotation::Up,
            Rotation::Left => Rotation::Right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ComponentOrientation {
    Up,
    Right,
    Down,
    Left,
    UpMirrored,
    RightMirrored,
    DownMirrored,
    LeftMirrored,
}

impl ComponentOrientation {
    pub const fn from_rotation(rotation: Rotation) -> Self {
        match rotation {
            Rotation::Up => Self::Up,
            Rotation::Right => Self::Right,
            Rotation::Down => Self::Down,
            Rotation::Left => Self::Left,
        }
    }

    pub const fn rotation(self) -> Rotation {
        match self {
            Self::Up | Self::UpMirrored => Rotation::Up,
            Self::Right | Self::RightMirrored => Rotation::Right,
            Self::Down | Self::DownMirrored => Rotation::Down,
            Self::Left | Self::LeftMirrored => Rotation::Left,
        }
    }

    pub const fn is_mirrored(self) -> bool {
        matches!(
            self,
            Self::UpMirrored | Self::RightMirrored | Self::DownMirrored | Self::LeftMirrored
        )
    }

    pub const fn swaps_axes(self) -> bool {
        self.rotation().swaps_axes()
    }

    pub const fn rotate_left(self) -> Self {
        match self {
            Self::Up => Self::Left,
            Self::Right => Self::Up,
            Self::Down => Self::Right,
            Self::Left => Self::Down,
            Self::UpMirrored => Self::RightMirrored,
            Self::RightMirrored => Self::DownMirrored,
            Self::DownMirrored => Self::LeftMirrored,
            Self::LeftMirrored => Self::UpMirrored,
        }
    }

    pub const fn rotate_right(self) -> Self {
        match self {
            Self::Up => Self::Right,
            Self::Right => Self::Down,
            Self::Down => Self::Left,
            Self::Left => Self::Up,
            Self::UpMirrored => Self::LeftMirrored,
            Self::RightMirrored => Self::UpMirrored,
            Self::DownMirrored => Self::RightMirrored,
            Self::LeftMirrored => Self::DownMirrored,
        }
    }

    pub const fn flip_horizontal(self) -> Self {
        match self {
            Self::Up => Self::UpMirrored,
            Self::Right => Self::RightMirrored,
            Self::Down => Self::DownMirrored,
            Self::Left => Self::LeftMirrored,
            Self::UpMirrored => Self::Up,
            Self::RightMirrored => Self::Right,
            Self::DownMirrored => Self::Down,
            Self::LeftMirrored => Self::Left,
        }
    }

    pub const fn flip_vertical(self) -> Self {
        match self {
            Self::Up => Self::DownMirrored,
            Self::Right => Self::LeftMirrored,
            Self::Down => Self::UpMirrored,
            Self::Left => Self::RightMirrored,
            Self::UpMirrored => Self::Down,
            Self::RightMirrored => Self::Left,
            Self::DownMirrored => Self::Up,
            Self::LeftMirrored => Self::Right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GeometryError {
    InvalidScale(u8),
    InvalidWire,
    InvalidSubcomponentSize(Size),
    InvalidSubcomponentPort { size: Size, port: ComponentPort },
    TooManySubcomponentPorts(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentKind {
    Not {
        scale: Scale,
    },
    MergerSplitter {
        input_scale: Scale,
        output_scale: Scale,
    },
    Led,
    Storage {
        scale: Scale,
        value: u64,
    },
    Input {
        scale: Scale,
        id: InputId,
        label: String,
    },
    Output {
        scale: Scale,
        id: OutputId,
        label: String,
    },
    Subcomponent {
        /// The compiled logic block whose program this subcomponent calls.
        compiled: Uuid,
        /// Display name of the subcomponent, shown in its centre. Copied from
        /// the compiled block's name when it is placed, so renaming the block
        /// later does not disturb grids already using it.
        name: String,
        size: Size,
        snap: Scale,
        ports: Vec<ComponentPort>,
        subgraphs: Vec<ComponentSubgraph>,
    },
}

impl ComponentKind {
    pub fn subcomponent(
        compiled: Uuid,
        size: Size,
        ports: Vec<ComponentPort>,
    ) -> Result<Self, GeometryError> {
        if size.width <= 0 || size.height <= 0 {
            return Err(GeometryError::InvalidSubcomponentSize(size));
        }
        if ports.len() > usize::from(u16::MAX) + 1 {
            return Err(GeometryError::TooManySubcomponentPorts(ports.len()));
        }
        if let Some(port) = ports.iter().find(|port| !port.is_valid_for(size)).cloned() {
            return Err(GeometryError::InvalidSubcomponentPort { size, port });
        }
        let snap = ports
            .iter()
            .map(|port| port.scale)
            .max()
            .unwrap_or(Scale::ONE);
        let subgraphs = vec![ComponentSubgraph::all_ports(&ports)];
        Ok(Self::Subcomponent {
            compiled,
            name: String::new(),
            size,
            snap,
            ports,
            subgraphs,
        })
    }

    pub fn subcomponent_with_subgraphs(
        compiled: Uuid,
        size: Size,
        ports: Vec<ComponentPort>,
        subgraphs: Vec<ComponentSubgraph>,
    ) -> Result<Self, GeometryError> {
        let mut kind = Self::subcomponent(compiled, size, ports)?;
        if let Self::Subcomponent {
            subgraphs: target, ..
        } = &mut kind
        {
            *target = subgraphs;
        }
        Ok(kind)
    }

    pub fn snap(&self) -> Scale {
        match self {
            Self::Not { scale }
            | Self::Storage { scale, .. }
            | Self::Input { scale, .. }
            | Self::Output { scale, .. } => *scale,
            Self::MergerSplitter {
                input_scale,
                output_scale,
            } => (*input_scale).max(*output_scale),
            Self::Led => Scale::ONE,
            Self::Subcomponent { snap, .. } => *snap,
        }
    }

    fn unrotated_size(&self) -> Option<Size> {
        match self {
            Self::Not { scale } | Self::Storage { scale, .. } => {
                let scale = scale.get();
                Some(Size::new(scale, scale.checked_mul(2)?))
            }
            Self::MergerSplitter {
                input_scale,
                output_scale,
            } => {
                let scale = input_scale.get().max(output_scale.get());
                Some(Size::new(scale, scale))
            }
            Self::Input { scale, .. } | Self::Output { scale, .. } => {
                let scale = scale.get();
                Some(Size::new(scale, scale))
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
            Self::Not { scale } => vec![
                ConnectionSlotDefinition::new(
                    0,
                    ConnectionDirection::Input,
                    ComponentSide::Bottom,
                    0,
                    size.width,
                    *scale,
                ),
                ConnectionSlotDefinition::new(
                    1,
                    ConnectionDirection::Output,
                    ComponentSide::Top,
                    0,
                    size.width,
                    *scale,
                ),
            ],
            Self::MergerSplitter {
                input_scale,
                output_scale,
            } => {
                let input_scale_value = input_scale.get();
                let output_scale_value = output_scale.get();
                let input_count = size.width / input_scale_value;
                let mut slots = (0..input_count)
                    .map(|index| {
                        ConnectionSlotDefinition::new(
                            index as u16,
                            ConnectionDirection::Input,
                            ComponentSide::Bottom,
                            index * input_scale_value,
                            (index + 1) * input_scale_value,
                            *input_scale,
                        )
                    })
                    .collect::<Vec<_>>();
                let output_count = size.width / output_scale_value;
                slots.extend((0..output_count).map(|index| {
                    ConnectionSlotDefinition::new(
                        (input_count + index) as u16,
                        ConnectionDirection::Output,
                        ComponentSide::Top,
                        index * output_scale_value,
                        (index + 1) * output_scale_value,
                        *output_scale,
                    )
                }));
                slots
            }
            Self::Led => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Input,
                ComponentSide::Bottom,
                0,
                size.width,
                Scale::ONE,
            )],
            Self::Storage { scale, .. } => vec![
                ConnectionSlotDefinition::new(
                    0,
                    ConnectionDirection::Input,
                    ComponentSide::Bottom,
                    0,
                    size.width,
                    *scale,
                ),
                ConnectionSlotDefinition::new(
                    1,
                    ConnectionDirection::Output,
                    ComponentSide::Top,
                    0,
                    size.width,
                    *scale,
                ),
            ],
            Self::Input { scale, .. } => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Output,
                ComponentSide::Bottom,
                0,
                size.width,
                *scale,
            )],
            Self::Output { scale, .. } => vec![ConnectionSlotDefinition::new(
                0,
                ConnectionDirection::Input,
                ComponentSide::Bottom,
                0,
                size.width,
                *scale,
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
                        port.scale,
                    )
                })
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct InputId(pub Uuid);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct OutputId(pub Uuid);

impl Default for InputId {
    fn default() -> Self {
        Self::new()
    }
}

impl InputId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }
}

impl Default for OutputId {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub const fn from_u128(value: u128) -> Self {
        Self(Uuid::from_u128(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub id: ComponentId,
    pub position: Point,
    pub orientation: ComponentOrientation,
    pub kind: ComponentKind,
}

impl Component {
    pub fn size(&self) -> Option<Size> {
        let size = self.kind.unrotated_size()?;
        if self.orientation.swaps_axes() {
            Some(Size::new(size.height, size.width))
        } else {
            Some(size)
        }
    }

    fn include_in_bounds(&self) -> bool {
        !matches!(
            self.kind,
            ComponentKind::Input { .. } | ComponentKind::Output { .. }
        )
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
                slot.transform_and_translate(unrotated_size, self.orientation, self.position)
            })
            .collect()
    }

    pub fn lead(&self) -> Option<ComponentLead> {
        if !matches!(
            self.kind,
            ComponentKind::Input { .. } | ComponentKind::Output { .. }
        ) {
            return None;
        }
        let size = self.kind.unrotated_size()?;
        let slot = ConnectionSlotDefinition::new(
            0,
            ConnectionDirection::Output,
            ComponentSide::Top,
            0,
            size.width,
            self.kind.snap(),
        )
        .transform_and_translate(size, self.orientation, self.position)?;
        let rect = self.rect()?;
        let axis = match slot.side {
            ComponentSide::Top => rect.min.y,
            ComponentSide::Right => rect.max.x,
            ComponentSide::Bottom => rect.max.y,
            ComponentSide::Left => rect.min.x,
        };
        Some(ComponentLead {
            side: slot.side,
            axis,
            start: slot.start,
            end: slot.end,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentLead {
    pub side: ComponentSide,
    pub axis: i64,
    pub start: i64,
    pub end: i64,
}

impl ComponentLead {
    fn intersects(self, rect: Rect) -> bool {
        match self.side {
            ComponentSide::Top => {
                self.start < rect.max.x && rect.min.x < self.end && rect.min.y < self.axis
            }
            ComponentSide::Right => {
                self.start < rect.max.y && rect.min.y < self.end && self.axis < rect.max.x
            }
            ComponentSide::Bottom => {
                self.start < rect.max.x && rect.min.x < self.end && self.axis < rect.max.y
            }
            ComponentSide::Left => {
                self.start < rect.max.y && rect.min.y < self.end && rect.min.x < self.axis
            }
        }
    }
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
    InfiniteLeadBlocked {
        component: ComponentId,
        blocker: ComponentId,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridBounds {
    pub min: Point,
    pub max: Point,
}

impl GridBounds {
    pub fn width(self) -> u128 {
        (self.max.x as i128 - self.min.x as i128) as u128
    }

    pub fn height(self) -> u128 {
        (self.max.y as i128 - self.min.y as i128) as u128
    }

    pub fn area(self) -> u128 {
        self.width() * self.height()
    }

    fn include(&mut self, rect: Rect) {
        self.min.x = self.min.x.min(rect.min.x);
        self.min.y = self.min.y.min(rect.min.y);
        self.max.x = self.max.x.max(rect.max.x);
        self.max.y = self.max.y.max(rect.max.y);
    }

    fn expand_to_grid(self, scale: Scale) -> Option<Self> {
        let scale = scale.get();
        Some(Self {
            min: Point::new(
                snap_min_to_grid(self.min.x, scale)?,
                snap_min_to_grid(self.min.y, scale)?,
            ),
            max: Point::new(
                snap_max_to_grid(self.max.x, scale)?,
                snap_max_to_grid(self.max.y, scale)?,
            ),
        })
    }
}

/// A grid of components and the wires between them. Serializing one writes the
/// same snapshot `snapshot` returns: the next component ID and the revision
/// counter are working state, not part of the circuit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LogicGrid {
    wires: Vec<Wire>,
    components: BTreeMap<ComponentId, Component>,
    next_component_id: u64,
    revision: u64,
}

impl Serialize for LogicGrid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.snapshot().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LogicGrid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self::from_snapshot(LogicGridSnapshot::deserialize(
            deserializer,
        )?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicGridSnapshot {
    pub components: Vec<Component>,
    pub wires: Vec<Wire>,
}

impl LogicGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn wires(&self) -> &[Wire] {
        &self.wires
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn snapshot(&self) -> LogicGridSnapshot {
        LogicGridSnapshot {
            components: self.components.values().cloned().collect(),
            wires: self.wires.clone(),
        }
    }

    pub fn from_snapshot(snapshot: LogicGridSnapshot) -> Self {
        let next_component_id = snapshot
            .components
            .iter()
            .map(|component| component.id.0)
            .max()
            .map_or(0, |id| id.saturating_add(1));
        let components = snapshot
            .components
            .into_iter()
            .map(|component| (component.id, component))
            .collect();

        Self {
            wires: snapshot.wires,
            components,
            next_component_id,
            revision: 0,
        }
    }

    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.components.values()
    }

    pub fn bounds(&self) -> Option<GridBounds> {
        let mut rects = self
            .components
            .values()
            .filter(|c| c.include_in_bounds())
            .map(Component::rect)
            .chain(self.wires.iter().copied().map(Wire::rect));
        let first = rects.next()??;
        let mut bounds = GridBounds {
            min: first.min,
            max: first.max,
        };
        for rect in rects {
            bounds.include(rect?);
        }
        match self.largest_io_scale() {
            Some(scale) => bounds.expand_to_grid(scale),
            None => Some(bounds),
        }
    }

    fn largest_io_scale(&self) -> Option<Scale> {
        self.components
            .values()
            .filter_map(|component| match component.kind {
                ComponentKind::Input { scale, .. } | ComponentKind::Output { scale, .. } => {
                    Some(scale)
                }
                _ => None,
            })
            .max()
    }

    pub fn component(&self, id: ComponentId) -> Option<&Component> {
        self.components.get(&id)
    }

    /// The ID `add_component` would hand out next. Callers that have to name a
    /// component before adding it - because the addition travels as an
    /// operation - allocate through this and `insert_component`.
    pub fn next_component_id(&self) -> ComponentId {
        ComponentId(self.next_component_id)
    }

    /// Adds a component under the ID it already carries. Returns `false` when
    /// that ID is taken, which is how an operation that has already been
    /// applied is ignored when it arrives again.
    pub fn insert_component(&mut self, component: Component) -> bool {
        if self.components.contains_key(&component.id) {
            return false;
        }
        self.next_component_id = self.next_component_id.max(component.id.0.saturating_add(1));
        self.components.insert(component.id, component);
        self.mark_changed();
        true
    }

    pub fn add_component(
        &mut self,
        position: Point,
        rotation: Rotation,
        kind: ComponentKind,
    ) -> ComponentId {
        self.add_component_with_orientation(
            position,
            ComponentOrientation::from_rotation(rotation),
            kind,
        )
    }

    pub fn add_component_with_orientation(
        &mut self,
        position: Point,
        orientation: ComponentOrientation,
        mut kind: ComponentKind,
    ) -> ComponentId {
        match &mut kind {
            ComponentKind::Storage { scale, value } => {
                *value &= value_mask(*scale);
            }
            ComponentKind::Input { id, .. } => {
                *id = InputId::new();
            }
            ComponentKind::Output { id, .. } => {
                *id = OutputId::new();
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
                orientation,
                kind,
            },
        );
        self.mark_changed();
        id
    }

    pub fn add_component_with_explicit_io(
        &mut self,
        position: Point,
        rotation: Rotation,
        mut kind: ComponentKind,
    ) -> ComponentId {
        if let ComponentKind::Storage { scale, value } = &mut kind {
            *value &= value_mask(*scale);
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
                orientation: ComponentOrientation::from_rotation(rotation),
                kind,
            },
        );
        self.mark_changed();
        id
    }

    pub fn remove_component(&mut self, id: ComponentId) -> Option<Component> {
        let removed = self.components.remove(&id);
        if removed.is_some() {
            self.mark_changed();
        }
        removed
    }

    pub fn set_component_position(&mut self, id: ComponentId, position: Point) -> bool {
        let Some(component) = self.components.get_mut(&id) else {
            return false;
        };
        if component.position == position {
            return false;
        }
        component.position = position;
        self.mark_changed();
        true
    }

    pub fn set_component_orientation(
        &mut self,
        id: ComponentId,
        orientation: ComponentOrientation,
    ) -> bool {
        let Some(component) = self.components.get_mut(&id) else {
            return false;
        };
        if component.orientation == orientation {
            return false;
        }
        component.orientation = orientation;
        self.mark_changed();
        true
    }

    pub fn set_component_kind(&mut self, id: ComponentId, mut kind: ComponentKind) -> bool {
        let Some(component) = self.components.get_mut(&id) else {
            return false;
        };
        if let ComponentKind::Storage { scale, value } = &mut kind {
            *value &= value_mask(*scale);
        }
        if component.kind == kind {
            return false;
        }
        component.kind = kind;
        self.mark_changed();
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
        let value = value & value_mask(*scale);
        if *stored == value {
            return false;
        }
        *stored = value;
        self.mark_changed();
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
        self.mark_changed();
        true
    }

    pub fn add_wire(&mut self, wire: Wire) -> &[Wire] {
        let original = self.wires.clone();
        self.wires.push(wire);
        self.normalize_wires();
        if self.wires != original {
            self.mark_changed();
        }
        &self.wires
    }

    pub fn remove_wire(&mut self, removal: Wire) -> &[Wire] {
        let original = self.wires.clone();
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
        if self.wires != original {
            self.mark_changed();
        }
        &self.wires
    }

    pub fn remove_wire_segment(&mut self, removal: Wire) -> &[Wire] {
        let original = self.wires.clone();
        if let Some(index) = self.wires.iter().position(|wire| *wire == removal) {
            self.wires.remove(index);
            self.normalize_wires();
            if self.wires != original {
                self.mark_changed();
            }
        }
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
            let Some(lead) = component.lead() else {
                continue;
            };
            for blocker in &components {
                if component.id != blocker.id
                    && blocker.rect().is_some_and(|rect| lead.intersects(rect))
                {
                    errors.insert(ValidationError::InfiniteLeadBlocked {
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
                if component
                    .rect()
                    .is_some_and(|component_rect| wire_rect.overlaps_area(component_rect))
                {
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

        let mut slot_infos: Vec<(ComponentId, ConnectionSlot, Rect)> = Vec::new();
        for component in self.components.values() {
            let Some(rect) = component.rect() else {
                continue;
            };
            for slot in component.connection_slots() {
                slot_infos.push((component.id, slot, rect));
            }
        }

        // Ports on touching components connect directly, forming wireless nets.
        let mut port_sets = DisjointSets::new(slot_infos.len());
        for first in 0..slot_infos.len() {
            for second in first + 1..slot_infos.len() {
                let (first_component, first_slot, first_rect) = slot_infos[first];
                let (second_component, second_slot, second_rect) = slot_infos[second];
                if first_component != second_component
                    && ports_connect(first_rect, first_slot, second_rect, second_slot)
                {
                    port_sets.union(first, second);
                }
            }
        }
        let mut port_groups = BTreeMap::<usize, Vec<usize>>::new();
        for index in 0..slot_infos.len() {
            port_groups
                .entry(port_sets.find(index))
                .or_default()
                .push(index);
        }
        let port_nets: Vec<Vec<usize>> = port_groups
            .into_values()
            .filter(|group| group.len() >= 2)
            .collect();
        let mut slot_port_net = vec![None; slot_infos.len()];
        for (net_index, group) in port_nets.iter().enumerate() {
            for &slot_index in group {
                slot_port_net[slot_index] = Some(net_index);
            }
        }

        let mut port_net_nodes = Vec::new();
        for _ in &port_nets {
            let node = GraphNodeId(nodes.len());
            nodes.push(GraphNode::WireNet { wires: Vec::new() });
            port_net_nodes.push(node);
        }

        let mut contacts = Vec::new();
        for (slot_index, &(component, slot, rect)) in slot_infos.iter().enumerate() {
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
            let port_net = slot_port_net[slot_index];
            if !connected_nets.is_empty() || port_net.is_some() {
                contacts.push((component, slot, connected_nets, port_net));
            }
        }
        contacts.sort();

        let mut edges = Vec::new();
        for (component, slot, connected_nets, port_net) in contacts {
            let connection = GraphNodeId(nodes.len());
            nodes.push(GraphNode::Connection {
                component,
                slot: slot.id,
                direction: slot.direction,
                side: slot.side,
                start: slot.start,
                end: slot.end,
                scale: slot.scale,
            });
            edges.push(GraphEdge::new(component_nodes[&component], connection));
            for net_index in connected_nets {
                edges.push(GraphEdge::new(connection, net_nodes[net_index]));
            }
            if let Some(port_net_index) = port_net {
                edges.push(GraphEdge::new(connection, port_net_nodes[port_net_index]));
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

    fn mark_changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
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
}

fn snap_min_to_grid(value: i64, scale: i64) -> Option<i64> {
    value.checked_sub(value.rem_euclid(scale))
}

fn snap_max_to_grid(value: i64, scale: i64) -> Option<i64> {
    let remainder = value.rem_euclid(scale);
    if remainder == 0 {
        Some(value)
    } else {
        value.checked_add(scale - remainder)
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ComponentSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ConnectionDirection {
    Input,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentPort {
    pub direction: ConnectionDirection,
    pub index: usize,
    pub scale: Scale,
    pub side: ComponentSide,
    pub start: i64,
    pub end: i64,
    /// Label shown next to this port when the component is used as a
    /// subcomponent. Carried over from the source input/output's label at
    /// compile time; empty when unlabelled.
    #[serde(default)]
    pub label: String,
}

impl ComponentPort {
    pub fn input(index: usize, scale: Scale, side: ComponentSide, start: i64, end: i64) -> Self {
        Self {
            direction: ConnectionDirection::Input,
            index,
            scale,
            side,
            start,
            end,
            label: String::new(),
        }
    }

    pub fn output(index: usize, scale: Scale, side: ComponentSide, start: i64, end: i64) -> Self {
        Self {
            direction: ConnectionDirection::Output,
            index,
            scale,
            side,
            start,
            end,
            label: String::new(),
        }
    }

    fn is_valid_for(&self, size: Size) -> bool {
        let boundary_length = match self.side {
            ComponentSide::Top | ComponentSide::Bottom => size.width,
            ComponentSide::Right | ComponentSide::Left => size.height,
        };
        0 <= self.start && self.start < self.end && self.end <= boundary_length
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComponentSubgraph {
    pub inputs: Vec<usize>,
    pub outputs: Vec<usize>,
}

impl ComponentSubgraph {
    pub fn all_ports(ports: &[ComponentPort]) -> Self {
        let mut inputs = ports
            .iter()
            .filter(|port| port.direction == ConnectionDirection::Input)
            .map(|port| port.index)
            .collect::<Vec<_>>();
        inputs.sort_unstable();
        inputs.dedup();
        let mut outputs = ports
            .iter()
            .filter(|port| port.direction == ConnectionDirection::Output)
            .map(|port| port.index)
            .collect::<Vec<_>>();
        outputs.sort_unstable();
        outputs.dedup();
        Self { inputs, outputs }
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
    pub scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ConnectionSlotDefinition {
    id: ConnectionSlotId,
    direction: ConnectionDirection,
    side: ComponentSide,
    start: i64,
    end: i64,
    scale: Scale,
}

impl ConnectionSlotDefinition {
    const fn new(
        id: u16,
        direction: ConnectionDirection,
        side: ComponentSide,
        start: i64,
        end: i64,
        scale: Scale,
    ) -> Self {
        Self {
            id: ConnectionSlotId(id),
            direction,
            side,
            start,
            end,
            scale,
        }
    }

    fn transform_and_translate(
        self,
        size: Size,
        orientation: ComponentOrientation,
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
        let first = transform_local_point(first, size, orientation)?;
        let second = transform_local_point(second, size, orientation)?;
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
                scale: self.scale,
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
                scale: self.scale,
            })
        } else {
            None
        }
    }
}

fn rotated_size(size: Size, orientation: ComponentOrientation) -> Size {
    if orientation.swaps_axes() {
        Size::new(size.height, size.width)
    } else {
        size
    }
}

fn transform_local_point(
    point: Point,
    size: Size,
    orientation: ComponentOrientation,
) -> Option<Point> {
    let rotated_size = rotated_size(size, orientation);
    let mut point = rotate_local_point(point, size, orientation.rotation())?;
    if orientation.is_mirrored() {
        point.x = rotated_size.width.checked_sub(point.x)?;
    }
    Some(point)
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
    scale: Scale,
}

impl Contact {
    fn overlaps(self, slot: ConnectionSlot) -> bool {
        self.scale == slot.scale
            && self.side == slot.side
            && self.start < slot.end
            && slot.start < self.end
    }
}

fn ports_connect(
    first_rect: Rect,
    first: ConnectionSlot,
    second_rect: Rect,
    second: ConnectionSlot,
) -> bool {
    if first.scale != second.scale || first.direction == second.direction {
        return false;
    }
    let touching = match (first.side, second.side) {
        (ComponentSide::Top, ComponentSide::Bottom) => first_rect.min.y == second_rect.max.y,
        (ComponentSide::Bottom, ComponentSide::Top) => first_rect.max.y == second_rect.min.y,
        (ComponentSide::Left, ComponentSide::Right) => first_rect.min.x == second_rect.max.x,
        (ComponentSide::Right, ComponentSide::Left) => first_rect.max.x == second_rect.min.x,
        _ => return false,
    };
    touching && first.start < second.end && second.start < first.end
}

// A port connects to any part of a wire that runs flush against the
// component's edge, regardless of which way the wire faces. The wire's whole
// body is considered, so the contact may fall anywhere along the wire rather
// than only at its endpoints.
fn wire_component_contacts(wire: Wire, component: Rect) -> Vec<Contact> {
    let Some(wire_rect) = wire.rect() else {
        return Vec::new();
    };
    let mut contacts = Vec::new();

    let vertical_start = wire_rect.min.y.max(component.min.y);
    let vertical_end = wire_rect.max.y.min(component.max.y);
    if vertical_start < vertical_end {
        if wire_rect.max.x == component.min.x {
            contacts.push(Contact {
                side: ComponentSide::Left,
                start: vertical_start,
                end: vertical_end,
                scale: wire.scale,
            });
        }
        if wire_rect.min.x == component.max.x {
            contacts.push(Contact {
                side: ComponentSide::Right,
                start: vertical_start,
                end: vertical_end,
                scale: wire.scale,
            });
        }
    }

    let horizontal_start = wire_rect.min.x.max(component.min.x);
    let horizontal_end = wire_rect.max.x.min(component.max.x);
    if horizontal_start < horizontal_end {
        if wire_rect.max.y == component.min.y {
            contacts.push(Contact {
                side: ComponentSide::Top,
                start: horizontal_start,
                end: horizontal_end,
                scale: wire.scale,
            });
        }
        if wire_rect.min.y == component.max.y {
            contacts.push(Contact {
                side: ComponentSide::Bottom,
                start: horizontal_start,
                end: horizontal_end,
                scale: wire.scale,
            });
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
        scale: Scale,
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
mod tests;
