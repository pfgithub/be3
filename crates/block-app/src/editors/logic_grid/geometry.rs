use super::*;

pub(super) fn snap_coordinate(value: f32, scale: Scale) -> i64 {
    let scale = scale.get();
    (value / scale as f32).floor() as i64 * scale
}

pub(super) fn snap_point(point: [f32; 2], scale: Scale) -> Point {
    Point::new(
        snap_coordinate(point[0], scale),
        snap_coordinate(point[1], scale),
    )
}

pub(super) fn snapped_delta(start: [f32; 2], end: [f32; 2], scale: Scale) -> Point {
    let scale = scale.get() as f32;
    Point::new(
        ((end[0] - start[0]) / scale).round() as i64 * scale as i64,
        ((end[1] - start[1]) / scale).round() as i64 * scale as i64,
    )
}

pub(super) fn translate_point(point: Point, delta: Point) -> Option<Point> {
    Some(Point::new(
        point.x.checked_add(delta.x)?,
        point.y.checked_add(delta.y)?,
    ))
}

pub(super) fn move_selected_wire(selected: SelectedWire, delta: Point) -> Option<Wire> {
    let start = if selected.start {
        translate_point(selected.wire.start, delta)?
    } else {
        selected.wire.start
    };
    let end = if selected.end {
        translate_point(selected.wire.end, delta)?
    } else {
        selected.wire.end
    };
    Wire::new(start, end, selected.wire.scale).ok()
}

pub(super) fn world_to_screen(world: [f32; 2], camera: Camera, rect: egui::Rect) -> egui::Pos2 {
    rect.center()
        + egui::vec2(
            (world[0] - camera.center[0]) * camera.zoom,
            (world[1] - camera.center[1]) * camera.zoom,
        )
}

#[derive(Clone, Copy)]
pub(super) struct WorldRect {
    min: [f32; 2],
    max: [f32; 2],
}

impl WorldRect {
    pub(super) fn from_points(first: [f32; 2], second: [f32; 2]) -> Self {
        Self {
            min: [first[0].min(second[0]), first[1].min(second[1])],
            max: [first[0].max(second[0]), first[1].max(second[1])],
        }
    }

    pub(super) fn intersects(self, min: [f32; 2], max: [f32; 2]) -> bool {
        self.min[0] <= max[0]
            && min[0] <= self.max[0]
            && self.min[1] <= max[1]
            && min[1] <= self.max[1]
    }
}

pub(super) fn component_contains(component: &Component, point: [f32; 2]) -> bool {
    let Some(size) = component.size() else {
        return false;
    };
    let (Some(right), Some(bottom)) = (
        component.position.x.checked_add(size.width),
        component.position.y.checked_add(size.height),
    ) else {
        return false;
    };
    point[0] >= component.position.x as f32
        && point[0] <= right as f32
        && point[1] >= component.position.y as f32
        && point[1] <= bottom as f32
}

pub(super) fn component_intersects(component: &Component, rect: WorldRect) -> bool {
    let Some(size) = component.size() else {
        return false;
    };
    let (Some(right), Some(bottom)) = (
        component.position.x.checked_add(size.width),
        component.position.y.checked_add(size.height),
    ) else {
        return false;
    };
    rect.intersects(
        [component.position.x as f32, component.position.y as f32],
        [right as f32, bottom as f32],
    )
}

pub(super) fn point_cell_intersects(point: Point, scale: Scale, rect: WorldRect) -> bool {
    let scale = scale.get() as f32;
    rect.intersects(
        [point.x as f32, point.y as f32],
        [point.x as f32 + scale, point.y as f32 + scale],
    )
}

pub(super) fn projected_wire(start: Point, end: Point, scale: Scale) -> Option<Wire> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let (start, end) = if dx.abs() >= dy.abs() {
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x);
        (Point::new(min_x, start.y), Point::new(max_x, start.y))
    } else {
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y);
        (Point::new(start.x, min_y), Point::new(start.x, max_y))
    };
    Wire::new(start, end, scale).ok()
}

pub(super) fn previous_scale(scale: Scale) -> Scale {
    let current = scale.get() as u8;
    let value = SCALES
        .iter()
        .copied()
        .rev()
        .find(|value| *value < current)
        .unwrap_or(current);
    Scale::new(value).expect("scale shortcut uses valid scale")
}

pub(super) fn next_scale(scale: Scale) -> Scale {
    let current = scale.get() as u8;
    let value = SCALES
        .iter()
        .copied()
        .find(|value| *value > current)
        .unwrap_or(current);
    Scale::new(value).expect("scale shortcut uses valid scale")
}

pub(super) fn placement_rotation(
    drag_start: [f32; 2],
    pointer: [f32; 2],
    selected: ComponentOrientation,
    kind: ToolKind,
) -> ComponentOrientation {
    match drag_rotation(drag_start, pointer) {
        Some(rotation) if kind == ToolKind::Input => {
            ComponentOrientation::from_rotation(rotation.flip())
        }
        Some(rotation) => ComponentOrientation::from_rotation(rotation),
        None => selected,
    }
}

pub(super) fn component_preview(
    tool: Tool,
    anchor: Point,
    orientation: ComponentOrientation,
    custom_kind: Option<&ComponentKind>,
) -> Option<Component> {
    let kind = match tool.kind {
        ToolKind::Select | ToolKind::Wire | ToolKind::ConfigureStorage => return None,
        ToolKind::Not => ComponentKind::Not { scale: tool.scale },
        ToolKind::MergerSplitter => {
            let (input_scale, output_scale) = tool.conversion_scales();
            ComponentKind::MergerSplitter {
                input_scale,
                output_scale,
            }
        }
        ToolKind::Led => ComponentKind::Led,
        ToolKind::Storage => ComponentKind::Storage {
            scale: tool.scale,
            value: 0,
        },
        ToolKind::Input => ComponentKind::Input {
            scale: tool.scale,
            id: InputId::from_u128(u128::MAX),
            label: String::new(),
        },
        ToolKind::Output => ComponentKind::Output {
            scale: tool.scale,
            id: OutputId::from_u128(u128::MAX),
            label: String::new(),
        },
        ToolKind::Custom => custom_kind.cloned()?,
    };
    let position = match tool.kind {
        ToolKind::Custom => subcomponent_placement_position(anchor, orientation, &kind)?,
        ToolKind::MergerSplitter => {
            let (_, output_scale) = tool.conversion_scales();
            component_placement_position(anchor, orientation.rotation(), output_scale, tool.kind)
        }
        ToolKind::Led => {
            component_placement_position(anchor, orientation.rotation(), Scale::ONE, tool.kind)
        }
        _ => component_placement_position(anchor, orientation.rotation(), tool.scale, tool.kind),
    };
    Some(Component {
        id: ComponentId(u64::MAX),
        position,
        orientation,
        kind,
    })
}

pub(super) fn drag_rotation(start: [f32; 2], pointer: [f32; 2]) -> Option<Rotation> {
    let dx = pointer[0] - start[0];
    let dy = pointer[1] - start[1];
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            Rotation::Right
        } else {
            Rotation::Left
        }
    } else if dy >= 0.0 {
        Rotation::Down
    } else {
        Rotation::Up
    })
}

pub(super) fn component_placement_position(
    anchor: Point,
    rotation: Rotation,
    scale: Scale,
    kind: ToolKind,
) -> Point {
    if matches!(
        kind,
        ToolKind::MergerSplitter | ToolKind::Input | ToolKind::Output
    ) {
        return anchor;
    }

    let scale = scale.get();
    match rotation {
        Rotation::Up => Point::new(anchor.x, anchor.y - scale),
        Rotation::Right | Rotation::Down => anchor,
        Rotation::Left => Point::new(anchor.x - scale, anchor.y),
    }
}

/// Places a custom component so the snapped local cell being held stays under
/// the cursor as the component rotates.
pub(super) fn subcomponent_placement_position(
    anchor: Point,
    orientation: ComponentOrientation,
    kind: &ComponentKind,
) -> Option<Point> {
    let ComponentKind::Subcomponent { size, .. } = kind else {
        return None;
    };
    let snap = kind.snap().get();
    let footprint = snapped_size(*size, snap)?;
    let offset = transform_local_snap_cell(Point::new(0, 0), footprint, snap, orientation)?;
    Some(Point::new(
        anchor.x.checked_sub(offset.x)?,
        anchor.y.checked_sub(offset.y)?,
    ))
}

fn snapped_size(size: logicgame::grid::Size, snap: i64) -> Option<logicgame::grid::Size> {
    Some(logicgame::grid::Size::new(
        snapped_extent(size.width, snap)?,
        snapped_extent(size.height, snap)?,
    ))
}

fn snapped_extent(extent: i64, snap: i64) -> Option<i64> {
    let remainder = extent.rem_euclid(snap);
    if remainder == 0 {
        Some(extent)
    } else {
        extent.checked_add(snap.checked_sub(remainder)?)
    }
}

fn transform_local_snap_cell(
    cell: Point,
    footprint: logicgame::grid::Size,
    snap: i64,
    orientation: ComponentOrientation,
) -> Option<Point> {
    let mut cell = rotate_local_snap_cell(cell, footprint, snap, orientation.rotation())?;
    if orientation.is_mirrored() {
        let width = if orientation.swaps_axes() {
            footprint.height
        } else {
            footprint.width
        };
        cell.x = width.checked_sub(cell.x.checked_add(snap)?)?;
    }
    Some(cell)
}

fn rotate_local_snap_cell(
    cell: Point,
    footprint: logicgame::grid::Size,
    snap: i64,
    rotation: Rotation,
) -> Option<Point> {
    match rotation {
        Rotation::Up => Some(cell),
        Rotation::Right => Some(Point::new(
            footprint.height.checked_sub(cell.y.checked_add(snap)?)?,
            cell.x,
        )),
        Rotation::Down => Some(Point::new(
            footprint.width.checked_sub(cell.x.checked_add(snap)?)?,
            footprint.height.checked_sub(cell.y.checked_add(snap)?)?,
        )),
        Rotation::Left => Some(Point::new(
            cell.y,
            footprint.width.checked_sub(cell.x.checked_add(snap)?)?,
        )),
    }
}

pub(super) fn nearest_wire(wires: &[Wire], point: [f32; 2], radius: f32) -> Option<Wire> {
    wires
        .iter()
        .copied()
        .filter_map(|wire| {
            let scale = wire.scale.get() as f32;
            let (min_x, max_x, min_y, max_y) = match wire.orientation() {
                logicgame::grid::Orientation::Horizontal => (
                    wire.start.x as f32,
                    wire.end.x as f32 + scale,
                    wire.start.y as f32,
                    wire.start.y as f32 + scale,
                ),
                logicgame::grid::Orientation::Vertical => (
                    wire.start.x as f32,
                    wire.start.x as f32 + scale,
                    wire.start.y as f32,
                    wire.end.y as f32 + scale,
                ),
            };
            let closest_x = point[0].clamp(min_x, max_x);
            let closest_y = point[1].clamp(min_y, max_y);
            let distance = ((point[0] - closest_x).powi(2) + (point[1] - closest_y).powi(2)).sqrt();
            (distance <= radius).then_some((distance, wire))
        })
        .min_by(|(first_distance, first), (second_distance, second)| {
            first_distance
                .total_cmp(second_distance)
                .then_with(|| first.cmp(second))
        })
        .map(|(_, wire)| wire)
}

pub(super) fn deletion_wire(wires: &[Wire], point: [f32; 2], radius: f32) -> Option<Wire> {
    directional_endpoint_wire(wires, point).or_else(|| nearest_wire(wires, point, radius))
}

fn directional_endpoint_wire(wires: &[Wire], point: [f32; 2]) -> Option<Wire> {
    wires
        .iter()
        .copied()
        .flat_map(|wire| [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end }))
        .filter_map(|endpoint| {
            let direction = pointer_direction_in_endpoint_cell(endpoint, point)?;
            (endpoint_direction(endpoint) == direction).then_some(endpoint.wire)
        })
        .min()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PointerDirection {
    Left,
    Right,
    Up,
    Down,
}

fn pointer_direction_in_endpoint_cell(
    endpoint: WireEndpoint,
    point: [f32; 2],
) -> Option<PointerDirection> {
    if !endpoint_cell_contains(endpoint, point) {
        return None;
    }
    let scale = endpoint.wire.scale.get() as f32;
    let endpoint_point = endpoint.point();
    let dx = point[0] - (endpoint_point.x as f32 + scale * 0.5);
    let dy = point[1] - (endpoint_point.y as f32 + scale * 0.5);
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        if dx >= 0.0 {
            PointerDirection::Right
        } else {
            PointerDirection::Left
        }
    } else if dy >= 0.0 {
        PointerDirection::Down
    } else {
        PointerDirection::Up
    })
}

fn endpoint_cell_contains(endpoint: WireEndpoint, point: [f32; 2]) -> bool {
    let scale = endpoint.wire.scale.get() as f32;
    let endpoint_point = endpoint.point();
    let min_x = endpoint_point.x as f32;
    let min_y = endpoint_point.y as f32;
    point[0] >= min_x && point[0] <= min_x + scale && point[1] >= min_y && point[1] <= min_y + scale
}

fn endpoint_direction(endpoint: WireEndpoint) -> PointerDirection {
    match (endpoint.wire.orientation(), endpoint.end) {
        (logicgame::grid::Orientation::Horizontal, WireEnd::Start) => PointerDirection::Right,
        (logicgame::grid::Orientation::Horizontal, WireEnd::End) => PointerDirection::Left,
        (logicgame::grid::Orientation::Vertical, WireEnd::Start) => PointerDirection::Down,
        (logicgame::grid::Orientation::Vertical, WireEnd::End) => PointerDirection::Up,
    }
}

pub(super) fn nearest_wire_endpoint(
    wires: &[Wire],
    point: [f32; 2],
    radius: f32,
) -> Option<WireEndpoint> {
    wires
        .iter()
        .copied()
        .flat_map(|wire| [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end }))
        .filter_map(|endpoint| {
            let scale = endpoint.wire.scale.get() as f32;
            let endpoint_point = endpoint.point();
            let center = [
                endpoint_point.x as f32 + scale * 0.5,
                endpoint_point.y as f32 + scale * 0.5,
            ];
            let distance = ((point[0] - center[0]).powi(2) + (point[1] - center[1]).powi(2)).sqrt();
            (distance <= radius + scale * 0.5).then_some((distance, endpoint))
        })
        .min_by(|(first_distance, first), (second_distance, second)| {
            first_distance
                .total_cmp(second_distance)
                .then_with(|| first.cmp(second))
        })
        .map(|(_, endpoint)| endpoint)
}
