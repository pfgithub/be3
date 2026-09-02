use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RotationDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScaleDirection {
    Down,
    Up,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FlipDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct SelectionBounds {
    min: Option<Point>,
    max: Option<Point>,
}

impl SelectionBounds {
    fn include_rect(&mut self, position: Point, size: logicgame::grid::Size) -> Option<()> {
        self.include(position);
        self.include(Point::new(
            position.x.checked_add(size.width)?,
            position.y.checked_add(size.height)?,
        ));
        Some(())
    }

    fn include_point_cell(&mut self, point: Point, scale: Scale) -> Option<()> {
        self.include_rect(point, logicgame::grid::Size::new(scale.get(), scale.get()))
    }

    fn include(&mut self, point: Point) {
        self.min = Some(match self.min {
            Some(min) => Point::new(min.x.min(point.x), min.y.min(point.y)),
            None => point,
        });
        self.max = Some(match self.max {
            Some(max) => Point::new(max.x.max(point.x), max.y.max(point.y)),
            None => point,
        });
    }

    fn non_empty(self) -> Option<Self> {
        match (self.min, self.max) {
            (Some(min), Some(max)) if min != max => Some(Self {
                min: Some(min),
                max: Some(max),
            }),
            _ => None,
        }
    }

    fn min(self) -> Point {
        self.min.expect("selection bounds has min")
    }
}

fn scaled_scale(scale: Scale, direction: ScaleDirection) -> Option<Scale> {
    let scaled = match direction {
        ScaleDirection::Down => previous_scale(scale),
        ScaleDirection::Up => next_scale(scale),
    };
    (scaled != scale).then_some(scaled)
}

fn scaled_component_kind(kind: &ComponentKind, direction: ScaleDirection) -> Option<ComponentKind> {
    Some(match kind {
        ComponentKind::Not { scale } => ComponentKind::Not {
            scale: scaled_scale(*scale, direction)?,
        },
        ComponentKind::MergerSplitter {
            input_scale,
            output_scale,
        } => ComponentKind::MergerSplitter {
            input_scale: scaled_scale(*input_scale, direction)?,
            output_scale: scaled_scale(*output_scale, direction)?,
        },
        ComponentKind::Storage { scale, value } => ComponentKind::Storage {
            scale: scaled_scale(*scale, direction)?,
            value: *value,
        },
        ComponentKind::Input { scale, id, label } => ComponentKind::Input {
            scale: scaled_scale(*scale, direction)?,
            id: *id,
            label: label.clone(),
        },
        ComponentKind::Output { scale, id, label } => ComponentKind::Output {
            scale: scaled_scale(*scale, direction)?,
            id: *id,
            label: label.clone(),
        },
        ComponentKind::Led | ComponentKind::Subcomponent { .. } => return None,
    })
}

fn scaled_point(point: Point, origin: Point, direction: ScaleDirection) -> Option<Point> {
    let dx = point.x.checked_sub(origin.x)?;
    let dy = point.y.checked_sub(origin.y)?;
    let (dx, dy) = match direction {
        ScaleDirection::Down => {
            if dx.rem_euclid(2) != 0 || dy.rem_euclid(2) != 0 {
                return None;
            }
            (dx / 2, dy / 2)
        }
        ScaleDirection::Up => (dx.checked_mul(2)?, dy.checked_mul(2)?),
    };
    Some(Point::new(
        origin.x.checked_add(dx)?,
        origin.y.checked_add(dy)?,
    ))
}

fn scale_transform_origin(
    bounds: SelectionBounds,
    direction: ScaleDirection,
    snap: Scale,
) -> Point {
    let min = bounds.min();
    match direction {
        ScaleDirection::Down => min,
        ScaleDirection::Up => Point::new(
            snap_coordinate(min.x as f32, snap),
            snap_coordinate(min.y as f32, snap),
        ),
    }
}

fn point_snapped_to_scale(point: Point, scale: Scale) -> bool {
    let scale = scale.get();
    point.x.rem_euclid(scale) == 0 && point.y.rem_euclid(scale) == 0
}

fn rotate_selected_wire(
    selected: SelectedWire,
    origin: Point,
    offset: Point,
    direction: RotationDirection,
) -> Option<Wire> {
    let start = if selected.start {
        rotate_point_cell(
            selected.wire.start,
            selected.wire.scale,
            origin,
            offset,
            direction,
        )?
    } else {
        selected.wire.start
    };
    let end = if selected.end {
        rotate_point_cell(
            selected.wire.end,
            selected.wire.scale,
            origin,
            offset,
            direction,
        )?
    } else {
        selected.wire.end
    };
    Wire::new(start, end, selected.wire.scale).ok()
}

fn rotate_rect_position(
    position: Point,
    size: logicgame::grid::Size,
    origin: Point,
    offset: Point,
    direction: RotationDirection,
) -> Option<Point> {
    let corners = [
        position,
        Point::new(position.x.checked_add(size.width)?, position.y),
        Point::new(
            position.x.checked_add(size.width)?,
            position.y.checked_add(size.height)?,
        ),
        Point::new(position.x, position.y.checked_add(size.height)?),
    ];
    corners
        .into_iter()
        .map(|point| rotate_point(point, origin, offset, direction))
        .collect::<Option<Vec<_>>>()
        .map(|points| {
            points
                .into_iter()
                .reduce(|min, point| Point::new(min.x.min(point.x), min.y.min(point.y)))
                .expect("rect has corners")
        })
}

fn rotate_point_cell(
    point: Point,
    scale: Scale,
    origin: Point,
    offset: Point,
    direction: RotationDirection,
) -> Option<Point> {
    rotate_rect_position(
        point,
        logicgame::grid::Size::new(scale.get(), scale.get()),
        origin,
        offset,
        direction,
    )
}

fn rotate_point(
    point: Point,
    origin: Point,
    offset: Point,
    direction: RotationDirection,
) -> Option<Point> {
    let dx = point.x.checked_sub(origin.x)?;
    let dy = point.y.checked_sub(origin.y)?;
    let (rotated_x, rotated_y) = match direction {
        RotationDirection::Left => (origin.x.checked_add(dy)?, origin.y.checked_sub(dx)?),
        RotationDirection::Right => (origin.x.checked_sub(dy)?, origin.y.checked_add(dx)?),
    };
    Some(Point::new(
        rotated_x.checked_add(offset.x)?,
        rotated_y.checked_add(offset.y)?,
    ))
}

fn rotation_offset(
    bounds: SelectionBounds,
    origin: Point,
    direction: RotationDirection,
) -> Option<Point> {
    let min = bounds.min.expect("selection bounds has min");
    let max = bounds.max.expect("selection bounds has max");
    let rotated_min = [min, Point::new(max.x, min.y), max, Point::new(min.x, max.y)]
        .into_iter()
        .map(|point| rotate_point(point, origin, Point::new(0, 0), direction))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .reduce(|min, point| Point::new(min.x.min(point.x), min.y.min(point.y)))
        .expect("bounds has corners");
    Some(Point::new(
        min.x.checked_sub(rotated_min.x)?,
        min.y.checked_sub(rotated_min.y)?,
    ))
}

fn aligned_rotation_offset(
    origin: Point,
    offset: Point,
    direction: RotationDirection,
    snap: Scale,
) -> Option<Point> {
    let rotated_zero = rotate_point(Point::new(0, 0), origin, offset, direction)?;
    let snap = snap.get();
    Some(Point::new(
        offset.x.checked_sub(rotated_zero.x.rem_euclid(snap))?,
        offset.y.checked_sub(rotated_zero.y.rem_euclid(snap))?,
    ))
}

fn flipped_component_orientation(
    orientation: ComponentOrientation,
    direction: FlipDirection,
) -> ComponentOrientation {
    match direction {
        FlipDirection::Horizontal => orientation.flip_horizontal(),
        FlipDirection::Vertical => orientation.flip_vertical(),
    }
}

fn flip_point_cell(
    point: Point,
    scale: Scale,
    bounds: SelectionBounds,
    direction: FlipDirection,
) -> Option<Point> {
    let min = bounds.min.expect("selection bounds has min");
    let max = bounds.max.expect("selection bounds has max");
    let scale = scale.get();
    Some(match direction {
        FlipDirection::Horizontal => Point::new(
            min.x
                .checked_add(max.x.checked_sub(point.x.checked_add(scale)?)?)?,
            point.y,
        ),
        FlipDirection::Vertical => Point::new(
            point.x,
            min.y
                .checked_add(max.y.checked_sub(point.y.checked_add(scale)?)?)?,
        ),
    })
}

fn flip_rect_position(
    position: Point,
    size: logicgame::grid::Size,
    bounds: SelectionBounds,
    direction: FlipDirection,
) -> Option<Point> {
    let min = bounds.min.expect("selection bounds has min");
    let max = bounds.max.expect("selection bounds has max");
    Some(match direction {
        FlipDirection::Horizontal => Point::new(
            min.x
                .checked_add(max.x.checked_sub(position.x.checked_add(size.width)?)?)?,
            position.y,
        ),
        FlipDirection::Vertical => Point::new(
            position.x,
            min.y
                .checked_add(max.y.checked_sub(position.y.checked_add(size.height)?)?)?,
        ),
    })
}

fn flip_selected_wire(
    selected: SelectedWire,
    bounds: SelectionBounds,
    direction: FlipDirection,
) -> Option<Wire> {
    let start = if selected.start {
        flip_point_cell(selected.wire.start, selected.wire.scale, bounds, direction)?
    } else {
        selected.wire.start
    };
    let end = if selected.end {
        flip_point_cell(selected.wire.end, selected.wire.scale, bounds, direction)?
    } else {
        selected.wire.end
    };
    Wire::new(start, end, selected.wire.scale).ok()
}

impl LogicGridEditor {
    pub(super) fn handle_canvas_input(&mut self, response: &egui::Response) {
        if self.tool.kind == ToolKind::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.delete_selection();
        }

        if self.tool.kind == ToolKind::Select && !response.ctx.egui_wants_keyboard_input() {
            response.ctx.input(|input| {
                if input.key_pressed(egui::Key::Q) {
                    self.rotate_selection(RotationDirection::Left);
                }
                if input.key_pressed(egui::Key::E) {
                    self.rotate_selection(RotationDirection::Right);
                }
                if input.key_pressed(egui::Key::OpenBracket) {
                    self.scale_selection(ScaleDirection::Down);
                }
                if input.key_pressed(egui::Key::CloseBracket) {
                    self.scale_selection(ScaleDirection::Up);
                }
                if input.key_pressed(egui::Key::H) {
                    self.flip_selection(FlipDirection::Horizontal);
                }
                if input.key_pressed(egui::Key::V) {
                    self.flip_selection(FlipDirection::Vertical);
                }
            });
        }

        if self.tool.kind.places_component() && !response.ctx.egui_wants_keyboard_input() {
            response.ctx.input(|input| {
                if input.key_pressed(egui::Key::Q) {
                    self.placement_orientation = self.placement_orientation.rotate_left();
                }
                if input.key_pressed(egui::Key::E) {
                    self.placement_orientation = self.placement_orientation.rotate_right();
                }
                if input.key_pressed(egui::Key::OpenBracket) {
                    self.tool.scale = previous_scale(self.tool.scale);
                }
                if input.key_pressed(egui::Key::CloseBracket) {
                    self.tool.scale = next_scale(self.tool.scale);
                }
                if input.key_pressed(egui::Key::H) {
                    self.placement_orientation = self.placement_orientation.flip_horizontal();
                }
                if input.key_pressed(egui::Key::V) {
                    self.placement_orientation = self.placement_orientation.flip_vertical();
                }
            });
        }

        if response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    self.camera
                        .zoom_around(pointer, response.rect, (scroll * 0.002).exp());
                }
            }
        }

        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };

        let middle_down = response
            .ctx
            .input(|input| input.pointer.button_down(PointerButton::Middle));
        if middle_down && response.hovered() {
            let delta = response.ctx.input(|input| input.pointer.delta());
            self.camera.center[0] -= delta.x / self.camera.zoom;
            self.camera.center[1] -= delta.y / self.camera.zoom;
            return;
        }

        let world = self.camera.screen_to_world(pointer, response.rect);
        let snapped = snap_point(world, self.active_tool_snap());

        if response.clicked_by(PointerButton::Secondary) && self.tool.kind == ToolKind::Wire {
            if let Some(wire) =
                deletion_wire(self.grid.wires(), world, WIRE_HIT_RADIUS / self.camera.zoom)
            {
                self.edit(LogicGridOperation::RemoveWireSegment { wire });
            }
        }

        let primary_pressed = response
            .ctx
            .input(|input| input.pointer.button_pressed(PointerButton::Primary));
        if primary_pressed && response.hovered() {
            let custom_kind = self.selected_custom_kind();
            self.gesture = match self.tool.kind {
                ToolKind::Select => {
                    let additive = response.ctx.input(|input| input.modifiers.shift);
                    let hit = self.entity_at(world);
                    match hit {
                        Some(entity) if additive => {
                            self.selection.toggle(entity);
                            None
                        }
                        Some(entity) => {
                            if !self.selection.contains(entity) {
                                self.selection.clear();
                                self.selection.insert(entity);
                            }
                            self.move_gesture(world)
                        }
                        None => Some(Gesture::SelectBox {
                            start: world,
                            additive,
                        }),
                    }
                }
                ToolKind::Wire => Some(Gesture::Wire { start: snapped }),
                ToolKind::Not => Some(Gesture::Not {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::MergerSplitter => Some(Gesture::MergerSplitter {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Led => Some(Gesture::Led {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Storage => Some(Gesture::Storage {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Input => (self.challenge.is_none()
                    || self.next_missing_challenge_input().is_some())
                .then_some(Gesture::Input {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::Output => (self.challenge.is_none()
                    || self.next_missing_challenge_output().is_some())
                .then_some(Gesture::Output {
                    anchor: snapped,
                    drag_start: world,
                }),
                ToolKind::ConfigureStorage => {
                    if let Some(DebugEntity::Component(id)) = self.entity_at(world) {
                        if let Some(ComponentKind::Storage { scale, .. }) =
                            self.grid.component(id).map(|component| &component.kind)
                        {
                            if *scale == Scale::ONE {
                                self.toggle_storage_bit(id, 0);
                            } else {
                                self.configured_storage = Some(id);
                            }
                        }
                    }
                    None
                }
                ToolKind::Custom => custom_kind.map(|kind| Gesture::Subcomponent {
                    anchor: snap_point(world, kind.snap()),
                    drag_start: world,
                    kind,
                }),
            };
        }

        let primary_released = response
            .ctx
            .input(|input| input.pointer.button_released(PointerButton::Primary));
        if primary_released {
            match self.gesture.take() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(start, snapped, self.tool.scale) {
                        self.edit(LogicGridOperation::AddWire { wire });
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Not,
                    );
                    let scale = self.tool.scale;
                    self.place(
                        component_placement_position(
                            anchor,
                            orientation.rotation(),
                            scale,
                            ToolKind::Not,
                        ),
                        orientation,
                        ComponentKind::Not { scale },
                    );
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::MergerSplitter,
                    );
                    let (input_scale, output_scale) = self.tool.conversion_scales();
                    self.place(
                        component_placement_position(
                            anchor,
                            orientation.rotation(),
                            output_scale,
                            ToolKind::MergerSplitter,
                        ),
                        orientation,
                        ComponentKind::MergerSplitter {
                            input_scale,
                            output_scale,
                        },
                    );
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Led,
                    );
                    self.place(
                        component_placement_position(
                            anchor,
                            orientation.rotation(),
                            Scale::ONE,
                            ToolKind::Led,
                        ),
                        orientation,
                        ComponentKind::Led,
                    );
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Storage,
                    );
                    let scale = self.tool.scale;
                    self.place(
                        component_placement_position(
                            anchor,
                            orientation.rotation(),
                            scale,
                            ToolKind::Storage,
                        ),
                        orientation,
                        ComponentKind::Storage { scale, value: 0 },
                    );
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Input,
                    );
                    let scale = self.active_input_scale();
                    let position = component_placement_position(
                        anchor,
                        orientation.rotation(),
                        scale,
                        ToolKind::Input,
                    );
                    if let Some(id) = self.add_input_at(position, orientation.rotation()) {
                        self.edit(LogicGridOperation::OrientComponent { id, orientation });
                    }
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Output,
                    );
                    let scale = self.active_output_scale();
                    let position = component_placement_position(
                        anchor,
                        orientation.rotation(),
                        scale,
                        ToolKind::Output,
                    );
                    if let Some(id) = self.add_output_at(position, orientation.rotation()) {
                        self.edit(LogicGridOperation::OrientComponent { id, orientation });
                    }
                }
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    let orientation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_orientation,
                        ToolKind::Custom,
                    );
                    if let Some(position) =
                        subcomponent_placement_position(anchor, orientation, &kind)
                    {
                        self.place(position, orientation, kind);
                    }
                }
                Some(Gesture::SelectBox { start, additive }) => {
                    if !additive {
                        self.selection.clear();
                    }
                    self.select_in_rect(start, world);
                }
                Some(Gesture::MoveSelection {
                    start,
                    scale,
                    components,
                    wires,
                }) => {
                    let delta = snapped_delta(start, world, scale);
                    self.apply_move(&components, &wires, delta);
                }
                None => {}
            }
        }
    }

    pub(super) fn entity_at(&self, point: [f32; 2]) -> Option<DebugEntity> {
        self.grid
            .components()
            .filter(|component| component_contains(component, point))
            .map(|component| DebugEntity::Component(component.id))
            .max_by_key(|entity| match entity {
                DebugEntity::Component(id) => *id,
                DebugEntity::Wire(_) | DebugEntity::WireEndpoint(_) => unreachable!(),
            })
            .or_else(|| {
                nearest_wire_endpoint(self.grid.wires(), point, WIRE_HIT_RADIUS / self.camera.zoom)
                    .map(DebugEntity::WireEndpoint)
            })
    }

    pub(super) fn move_gesture(&self, start: [f32; 2]) -> Option<Gesture> {
        if self.selection.is_empty() {
            return None;
        }
        let components: Vec<_> = self
            .selection
            .components
            .iter()
            .filter_map(|id| {
                self.grid
                    .component(*id)
                    .map(|component| (*id, component.position))
            })
            .collect();
        let wires = self.selection.selected_wires();
        let scale = components
            .iter()
            .filter_map(|(id, _)| self.grid.component(*id))
            .map(|component| component.kind.snap())
            .chain(wires.iter().map(|wire| wire.wire.scale))
            .max()
            .unwrap_or(Scale::ONE);
        Some(Gesture::MoveSelection {
            start,
            scale,
            components,
            wires,
        })
    }

    pub(super) fn select_in_rect(&mut self, start: [f32; 2], end: [f32; 2]) {
        let rect = WorldRect::from_points(start, end);
        self.selection.components.extend(
            self.grid
                .components()
                .filter(|component| component_intersects(component, rect))
                .map(|component| component.id),
        );
        for wire in self.grid.wires().iter().copied() {
            for end in [WireEnd::Start, WireEnd::End] {
                let endpoint = WireEndpoint { wire, end };
                if point_cell_intersects(endpoint.point(), wire.scale, rect) {
                    self.selection.wire_endpoints.insert(endpoint);
                }
            }
        }
    }

    pub(super) fn apply_move(
        &mut self,
        components: &[(ComponentId, Point)],
        wires: &[SelectedWire],
        delta: Point,
    ) {
        if delta == Point::new(0, 0) {
            return;
        }
        let moved_components: Option<Vec<_>> = components
            .iter()
            .map(|(id, position)| translate_point(*position, delta).map(|point| (*id, point)))
            .collect();
        let moved_wires: Vec<_> = wires
            .iter()
            .filter_map(|wire| move_selected_wire(*wire, delta))
            .collect();
        let Some(moved_components) = moved_components else {
            return;
        };

        let mut operations = Vec::new();
        for wire in wires {
            operations.push(LogicGridOperation::RemoveWire { wire: wire.wire });
        }
        for (id, position) in moved_components {
            operations.push(LogicGridOperation::MoveComponent { id, position });
        }
        for wire in &moved_wires {
            operations.push(LogicGridOperation::AddWire { wire: *wire });
        }
        self.edit_all(operations);

        let selected_points: BTreeSet<_> = wires
            .iter()
            .filter_map(|wire| {
                move_selected_wire(*wire, delta).map(|moved| {
                    let mut points = Vec::new();
                    if wire.start {
                        points.push((
                            translate_point(wire.wire.start, delta).unwrap(),
                            moved.scale,
                        ));
                    }
                    if wire.end {
                        points.push((translate_point(wire.wire.end, delta).unwrap(), moved.scale));
                    }
                    points
                })
            })
            .flatten()
            .collect();
        self.selection.wire_endpoints = self
            .grid
            .wires()
            .iter()
            .copied()
            .flat_map(|wire| {
                [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end })
            })
            .filter(|endpoint| selected_points.contains(&(endpoint.point(), endpoint.wire.scale)))
            .collect();
    }

    pub(super) fn rotate_selection(&mut self, direction: RotationDirection) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let Some(bounds) = self.selection_bounds() else {
            return false;
        };
        let origin = bounds.min();
        let Some(offset) = rotation_offset(bounds, origin, direction) else {
            return false;
        };
        let wires = self.selection.selected_wires();
        let snap = self
            .selection
            .components
            .iter()
            .filter_map(|id| self.grid.component(*id))
            .map(|component| component.kind.snap())
            .chain(wires.iter().map(|wire| wire.wire.scale))
            .max()
            .unwrap_or(Scale::ONE);
        let Some(offset) = aligned_rotation_offset(origin, offset, direction, snap) else {
            return false;
        };
        let components: Option<Vec<_>> = self
            .selection
            .components
            .iter()
            .map(|id| {
                let component = self.grid.component(*id)?;
                let size = component.size()?;
                let orientation = match direction {
                    RotationDirection::Left => component.orientation.rotate_left(),
                    RotationDirection::Right => component.orientation.rotate_right(),
                };
                let position =
                    rotate_rect_position(component.position, size, origin, offset, direction)
                        .filter(|point| point_snapped_to_scale(*point, component.kind.snap()))?;
                Some((*id, position, orientation))
            })
            .collect();
        let Some(components) = components else {
            return false;
        };

        let rotated_wires: Option<Vec<_>> = wires
            .iter()
            .map(|wire| rotate_selected_wire(*wire, origin, offset, direction))
            .collect();
        let Some(rotated_wires) = rotated_wires else {
            return false;
        };

        let mut operations = Vec::new();
        for wire in &wires {
            operations.push(LogicGridOperation::RemoveWire { wire: wire.wire });
        }
        for (id, position, orientation) in components {
            operations.push(LogicGridOperation::MoveComponent { id, position });
            operations.push(LogicGridOperation::OrientComponent { id, orientation });
        }
        for wire in &rotated_wires {
            operations.push(LogicGridOperation::AddWire { wire: *wire });
        }
        self.edit_all(operations);
        self.reselect_wire_points(
            wires
                .iter()
                .zip(rotated_wires.iter())
                .flat_map(|(selected, rotated)| {
                    let mut points = Vec::new();
                    if selected.start {
                        points.push((rotated.start, rotated.scale));
                    }
                    if selected.end {
                        points.push((rotated.end, rotated.scale));
                    }
                    points
                })
                .collect(),
        );
        true
    }

    pub(super) fn flip_selection(&mut self, direction: FlipDirection) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let Some(bounds) = self.selection_bounds() else {
            return false;
        };
        let components: Option<Vec<_>> = self
            .selection
            .components
            .iter()
            .map(|id| {
                let component = self.grid.component(*id)?;
                let size = component.size()?;
                let position = flip_rect_position(component.position, size, bounds, direction)?;
                Some((
                    *id,
                    position,
                    flipped_component_orientation(component.orientation, direction),
                ))
            })
            .collect();
        let Some(components) = components else {
            return false;
        };

        let wires = self.selection.selected_wires();
        let flipped_wires: Option<Vec<_>> = wires
            .iter()
            .map(|wire| flip_selected_wire(*wire, bounds, direction))
            .collect();
        let Some(flipped_wires) = flipped_wires else {
            return false;
        };

        let mut operations = Vec::new();
        for wire in &wires {
            operations.push(LogicGridOperation::RemoveWire { wire: wire.wire });
        }
        for (id, position, orientation) in components {
            operations.push(LogicGridOperation::MoveComponent { id, position });
            operations.push(LogicGridOperation::OrientComponent { id, orientation });
        }
        for wire in &flipped_wires {
            operations.push(LogicGridOperation::AddWire { wire: *wire });
        }
        self.edit_all(operations);
        self.reselect_wire_points(
            wires
                .iter()
                .zip(flipped_wires.iter())
                .flat_map(|(selected, flipped)| {
                    let mut points = Vec::new();
                    if selected.start {
                        points.push((flipped.start, flipped.scale));
                    }
                    if selected.end {
                        points.push((flipped.end, flipped.scale));
                    }
                    points
                })
                .collect(),
        );
        true
    }

    pub(super) fn scale_selection(&mut self, direction: ScaleDirection) -> bool {
        if self.selection.is_empty() {
            return false;
        }
        let Some(bounds) = self.selection_bounds() else {
            return false;
        };
        let components: Option<Vec<_>> = self
            .selection
            .components
            .iter()
            .map(|id| {
                let component = self.grid.component(*id)?;
                let kind = scaled_component_kind(&component.kind, direction)?;
                Some((*id, component.position, kind))
            })
            .collect();
        let Some(components) = components else {
            return false;
        };

        let wires = self.selection.selected_wires();
        let snap = components
            .iter()
            .map(|(_, _, kind)| kind.snap())
            .chain(
                wires
                    .iter()
                    .filter_map(|wire| scaled_scale(wire.wire.scale, direction)),
            )
            .max()
            .unwrap_or(Scale::ONE);
        let origin = scale_transform_origin(bounds, direction, snap);
        let scaled_components: Option<Vec<_>> = components
            .iter()
            .map(|(id, position, kind)| {
                let position = scaled_point(*position, origin, direction)?;
                point_snapped_to_scale(position, kind.snap()).then_some((
                    *id,
                    position,
                    kind.clone(),
                ))
            })
            .collect();
        let Some(scaled_components) = scaled_components else {
            return false;
        };
        let scaled_wires: Option<Vec<_>> = wires
            .iter()
            .map(|wire| {
                let scale = scaled_scale(wire.wire.scale, direction)?;
                let start = scaled_point(wire.wire.start, origin, direction)?;
                let end = scaled_point(wire.wire.end, origin, direction)?;
                if !point_snapped_to_scale(start, scale) || !point_snapped_to_scale(end, scale) {
                    return None;
                }
                Wire::new(start, end, scale).ok()
            })
            .collect();
        let Some(scaled_wires) = scaled_wires else {
            return false;
        };

        let mut operations = Vec::new();
        for wire in &wires {
            operations.push(LogicGridOperation::RemoveWire { wire: wire.wire });
        }
        for (id, position, kind) in scaled_components {
            operations.push(LogicGridOperation::MoveComponent { id, position });
            operations.push(LogicGridOperation::SetComponentKind { id, kind });
        }
        for wire in &scaled_wires {
            operations.push(LogicGridOperation::AddWire { wire: *wire });
        }
        self.edit_all(operations);
        self.reselect_wire_points(
            wires
                .iter()
                .zip(scaled_wires.iter())
                .flat_map(|(selected, scaled)| {
                    let mut points = Vec::new();
                    if selected.start {
                        points.push((scaled.start, scaled.scale));
                    }
                    if selected.end {
                        points.push((scaled.end, scaled.scale));
                    }
                    points
                })
                .collect(),
        );
        true
    }

    fn selection_bounds(&self) -> Option<SelectionBounds> {
        let mut bounds = SelectionBounds::default();
        for id in &self.selection.components {
            let component = self.grid.component(*id)?;
            bounds.include_rect(component.position, component.size()?)?;
        }
        for endpoint in &self.selection.wire_endpoints {
            bounds.include_point_cell(endpoint.point(), endpoint.wire.scale)?;
        }
        bounds.non_empty()
    }

    fn reselect_wire_points(&mut self, selected_points: BTreeSet<(Point, Scale)>) {
        self.selection.wire_endpoints = self
            .grid
            .wires()
            .iter()
            .copied()
            .flat_map(|wire| {
                [WireEnd::Start, WireEnd::End].map(move |end| WireEndpoint { wire, end })
            })
            .filter(|endpoint| selected_points.contains(&(endpoint.point(), endpoint.wire.scale)))
            .collect();
    }

    pub(super) fn delete_selection(&mut self) {
        let mut operations = std::mem::take(&mut self.selection.components)
            .into_iter()
            .map(|id| LogicGridOperation::RemoveComponent { id })
            .collect::<Vec<_>>();
        let wires: BTreeSet<_> = std::mem::take(&mut self.selection.wire_endpoints)
            .into_iter()
            .map(|endpoint| endpoint.wire)
            .collect();
        operations.extend(
            wires
                .into_iter()
                .map(|wire| LogicGridOperation::RemoveWire { wire }),
        );
        self.edit_all(operations);
        self.gesture = None;
    }

    pub(super) fn selected_custom_kind(&self) -> Option<ComponentKind> {
        (self.tool.kind == ToolKind::Custom)
            .then_some(self.active_hotbar_slot.as_deref())
            .flatten()
            .and_then(|path| self.selected_hotbar_kind(path))
    }

    pub(super) fn placement_preview_component(&self, pointer: [f32; 2]) -> Option<Component> {
        if let Some(kind) = self.selected_custom_kind() {
            return component_preview(
                self.tool,
                snap_point(pointer, kind.snap()),
                self.placement_orientation,
                Some(&kind),
            );
        }
        let mut tool = self.tool;
        tool.scale = match self.tool.kind {
            ToolKind::Input => {
                if self.challenge.is_some() && self.next_missing_challenge_input().is_none() {
                    return None;
                }
                self.active_input_scale()
            }
            ToolKind::Output => {
                if self.challenge.is_some() && self.next_missing_challenge_output().is_none() {
                    return None;
                }
                self.active_output_scale()
            }
            _ => self.tool.scale,
        };
        component_preview(
            tool,
            snap_point(pointer, self.active_tool_snap()),
            self.placement_orientation,
            None,
        )
    }
}
