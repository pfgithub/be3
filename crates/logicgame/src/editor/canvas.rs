use super::*;

impl LogicEditor {
    pub(super) fn handle_canvas_input(&mut self, response: &egui::Response) {
        if self.tool.kind == ToolKind::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.delete_selection();
        }

        if self.tool.kind.places_component() && !response.ctx.egui_wants_keyboard_input() {
            response.ctx.input(|input| {
                if input.key_pressed(egui::Key::Q) {
                    self.placement_rotation = rotate_left(self.placement_rotation);
                }
                if input.key_pressed(egui::Key::E) {
                    self.placement_rotation = rotate_right(self.placement_rotation);
                }
                if input.key_pressed(egui::Key::OpenBracket) {
                    self.tool.scale = previous_scale(self.tool.scale);
                }
                if input.key_pressed(egui::Key::CloseBracket) {
                    self.tool.scale = next_scale(self.tool.scale);
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
                nearest_wire(self.grid.wires(), world, WIRE_HIT_RADIUS / self.camera.zoom)
            {
                self.grid.remove_wire(wire);
            }
        }

        let primary_pressed = response
            .ctx
            .input(|input| input.pointer.button_pressed(PointerButton::Primary));
        if primary_pressed && response.hovered() {
            // The custom component to place, resolved before the match so the
            // arm does not borrow `self` while assigning `self.gesture`.
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
                ToolKind::Input => Some(Gesture::Input {
                    anchor: snapped,
                    drag_start: world,
                })
                .filter(|_| {
                    self.challenge.is_none() || self.next_missing_challenge_input().is_some()
                }),
                ToolKind::Output => Some(Gesture::Output {
                    anchor: snapped,
                    drag_start: world,
                })
                .filter(|_| {
                    self.challenge.is_none() || self.next_missing_challenge_output().is_some()
                }),
                ToolKind::ConfigureStorage => {
                    if let Some(DebugEntity::Component(id)) = self.entity_at(world) {
                        if let Some(ComponentKind::Storage { scale, .. }) =
                            self.grid.component(id).map(|component| &component.kind)
                        {
                            if *scale == Scale::ONE {
                                self.grid.toggle_storage_bit(id, 0);
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
                        self.grid.add_wire(wire);
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Not,
                    );
                    let scale = self.tool.scale;
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, scale, ToolKind::Not),
                        rotation,
                        ComponentKind::Not { scale },
                    );
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::MergerSplitter,
                    );
                    let (input_scale, output_scale) = self.tool.conversion_scales();
                    self.grid.add_component(
                        component_placement_position(
                            anchor,
                            rotation,
                            output_scale,
                            ToolKind::MergerSplitter,
                        ),
                        rotation,
                        ComponentKind::MergerSplitter {
                            input_scale,
                            output_scale,
                        },
                    );
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Led,
                    );
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, Scale::ONE, ToolKind::Led),
                        rotation,
                        ComponentKind::Led,
                    );
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Storage,
                    );
                    let scale = self.tool.scale;
                    self.grid.add_component(
                        component_placement_position(anchor, rotation, scale, ToolKind::Storage),
                        rotation,
                        ComponentKind::Storage { scale, value: 0 },
                    );
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Input,
                    );
                    let scale = self.active_input_scale();
                    let position =
                        component_placement_position(anchor, rotation, scale, ToolKind::Input);
                    self.add_input_at(position, rotation);
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Output,
                    );
                    let scale = self.active_output_scale();
                    let position =
                        component_placement_position(anchor, rotation, scale, ToolKind::Output);
                    self.add_output_at(position, rotation);
                }
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    let rotation = placement_rotation(
                        drag_start,
                        world,
                        self.placement_rotation,
                        ToolKind::Custom,
                    );
                    let position = subcomponent_placement_position(anchor, rotation, &kind);
                    self.grid.add_component(position, rotation, kind);
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

        for wire in wires {
            self.grid.remove_wire(wire.wire);
        }
        for (id, position) in moved_components {
            self.grid.set_component_position(id, position);
        }
        for wire in &moved_wires {
            self.grid.add_wire(*wire);
        }

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

    pub(super) fn delete_selection(&mut self) {
        for id in std::mem::take(&mut self.selection.components) {
            self.grid.remove_component(id);
        }
        let wires: BTreeSet<_> = std::mem::take(&mut self.selection.wire_endpoints)
            .into_iter()
            .map(|endpoint| endpoint.wire)
            .collect();
        for wire in wires {
            self.grid.remove_wire(wire);
        }
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
                self.placement_rotation,
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
            self.placement_rotation,
            None,
        )
    }
}
