use super::*;

impl LogicEditor {
    /// Draws text labels over the grid: the label on each input/output, the
    /// centre name of every subcomponent, and each subcomponent port's label
    /// next to its port. Text is an egui overlay because the wgpu grid renderer
    /// only draws triangles.
    pub(super) fn draw_component_labels(&self, painter: &egui::Painter, rect: egui::Rect) {
        let zoom = self.camera.zoom;
        for component in self.grid.components() {
            let Some(size) = component.size() else {
                continue;
            };
            let center = [
                component.position.x as f32 + size.width as f32 * 0.5,
                component.position.y as f32 + size.height as f32 * 0.5,
            ];
            match &component.kind {
                ComponentKind::Input { label, .. } | ComponentKind::Output { label, .. } => {
                    if label.is_empty() {
                        continue;
                    }
                    painter.text(
                        world_to_screen(center, self.camera, rect),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional((zoom * 0.5).clamp(7.0, 28.0)),
                        LABEL_COLOR,
                    );
                }
                ComponentKind::Subcomponent { name, ports, .. } => {
                    if !name.is_empty() {
                        painter.text(
                            world_to_screen(center, self.camera, rect),
                            egui::Align2::CENTER_CENTER,
                            name,
                            egui::FontId::proportional((zoom * 0.45).clamp(8.0, 30.0)),
                            NAME_COLOR,
                        );
                    }
                    for slot in component.connection_slots() {
                        let Some(port) = ports.get(slot.id.0 as usize) else {
                            continue;
                        };
                        if port.label.is_empty() {
                            continue;
                        }
                        let mid = (slot.start + slot.end) as f32 * 0.5;
                        let left = component.position.x as f32;
                        let top = component.position.y as f32;
                        let right = left + size.width as f32;
                        let bottom = top + size.height as f32;
                        // Sit the text just inside the edge, anchored so it grows
                        // toward the component's interior.
                        let inset = 0.15;
                        let (point, anchor) = match slot.side {
                            ComponentSide::Top => ([mid, top + inset], egui::Align2::CENTER_TOP),
                            ComponentSide::Bottom => {
                                ([mid, bottom - inset], egui::Align2::CENTER_BOTTOM)
                            }
                            ComponentSide::Left => ([left + inset, mid], egui::Align2::LEFT_CENTER),
                            ComponentSide::Right => {
                                ([right - inset, mid], egui::Align2::RIGHT_CENTER)
                            }
                        };
                        painter.text(
                            world_to_screen(point, self.camera, rect),
                            anchor,
                            &port.label,
                            egui::FontId::proportional((zoom * 0.32).clamp(6.0, 16.0)),
                            PORT_LABEL_COLOR,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    pub(super) fn wire_value_indices(
        &self,
        snapshot: &SimulationSnapshot,
    ) -> (
        BTreeMap<Wire, u32>,
        BTreeMap<(ComponentId, ConnectionSlot), u32>,
        BTreeMap<ComponentId, u32>,
        BTreeMap<ComponentId, u32>,
        Vec<WireValue>,
    ) {
        let simulation_vm = self
            .simulation
            .snapshot
            .as_ref()
            .filter(|simulation_snapshot| *simulation_snapshot == snapshot)
            .and(self.simulation.vm.as_ref());
        let root_memory = simulation_vm.map(Vm::root_memory).unwrap_or_default();

        let mut values = Vec::new();
        let mut indices = BTreeMap::new();
        let mut net_value: BTreeMap<usize, u32> = BTreeMap::new();
        let mut address = 0;
        for (node_index, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::WireNet { wires } = node else {
                continue;
            };
            let value_index = values.len() as u32;
            net_value.insert(node_index, value_index);
            values.push(WireValue::new(
                root_memory.get(address).copied().unwrap_or_default(),
            ));
            for wire in wires {
                indices.insert(*wire, value_index);
            }
            address += 1;
        }

        let mut component_indices: BTreeMap<ComponentId, u32> = BTreeMap::new();
        let mut connection_indices: BTreeMap<(ComponentId, ConnectionSlot), u32> = BTreeMap::new();
        for (i, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::Connection {
                component,
                slot,
                direction,
                side,
                start,
                end,
                scale,
            } = node
            else {
                continue;
            };
            for edge in &snapshot.graph.edges {
                let other = if edge.first == GraphNodeId(i) {
                    Some(edge.second.0)
                } else if edge.second == GraphNodeId(i) {
                    Some(edge.first.0)
                } else {
                    None
                };
                if let Some(vi) = other.and_then(|j| net_value.get(&j)) {
                    connection_indices.insert(
                        (
                            *component,
                            ConnectionSlot {
                                id: *slot,
                                direction: *direction,
                                side: *side,
                                start: *start,
                                end: *end,
                                scale: *scale,
                            },
                        ),
                        *vi,
                    );
                    component_indices.insert(*component, *vi);
                    break;
                }
            }
        }

        let mut storage_indices = BTreeMap::new();
        for (storage_index, component) in self
            .grid
            .components()
            .filter(|component| matches!(component.kind, ComponentKind::Storage { .. }))
            .enumerate()
        {
            let ComponentKind::Storage { value, .. } = component.kind else {
                continue;
            };
            let value = simulation_vm
                .and_then(|vm| vm.storage.get(storage_index).copied())
                .unwrap_or(value);
            let value_index = values.len() as u32;
            values.push(WireValue::new(value));
            storage_indices.insert(component.id, value_index);
        }

        if values.is_empty() {
            values.push(WireValue::new(0));
        }
        (
            indices,
            connection_indices,
            component_indices,
            storage_indices,
            values,
        )
    }

    pub(super) fn show_storage_configuration(&mut self, context: &egui::Context) {
        let Some(id) = self.configured_storage else {
            return;
        };
        let Some((scale, value)) = self.grid.component(id).and_then(|component| {
            if let ComponentKind::Storage { scale, value } = &component.kind {
                Some((*scale, *value))
            } else {
                None
            }
        }) else {
            self.configured_storage = None;
            return;
        };

        let mut open = true;
        egui::Window::new(format!("Configure storage #{}", id.0))
            .open(&mut open)
            .default_width(180.0)
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                for bit in storage_bit_indices(scale) {
                    let state = (value >> bit) & 1;
                    if ui.button(format!("Bit {bit}: {state}")).clicked() {
                        self.grid.toggle_storage_bit(id, bit);
                    }
                }
            });
        if !open {
            self.configured_storage = None;
        }
    }

    pub(super) fn show_grid_debugger(
        &self,
        context: &egui::Context,
        hovered_square: Option<Point>,
    ) -> Option<DebugEntity> {
        let errors = self.grid.validate();
        let bounds = self.grid.bounds();
        let mut hovered_entity = None;

        egui::Window::new("Grid Debugger")
            .default_pos([700.0, 260.0])
            .default_width(240.0)
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                egui::Grid::new("logic-grid-debug-summary")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Components");
                        ui.monospace(self.grid.components().count().to_string());
                        ui.end_row();
                        ui.label("Wires");
                        ui.monospace(self.grid.wires().len().to_string());
                        ui.end_row();
                        ui.label("Bounds");
                        match bounds {
                            Some(bounds) => {
                                ui.monospace(format!(
                                    "({}, {}) to ({}, {})",
                                    bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y
                                ));
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();
                        ui.label("Validation errors");
                        ui.monospace(errors.len().to_string());
                        ui.end_row();
                        ui.label("Hovered square");
                        match hovered_square {
                            Some(point) => {
                                ui.monospace(format!("({}, {})", point.x, point.y));
                            }
                            None => {
                                ui.weak("none");
                            }
                        }
                        ui.end_row();
                    });

                ui.separator();
                ui.strong("Validation errors");
                if errors.is_empty() {
                    ui.weak("No validation errors");
                } else {
                    egui::ScrollArea::both()
                        .id_salt("logic-grid-validation-errors")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for (index, error) in errors.iter().enumerate() {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!("#{index} {error:#?}"))
                                            .monospace(),
                                    )
                                    .selectable(true),
                                );
                            }
                        });
                }

                ui.separator();
                ui.strong("Entities");
                if self.grid.components().next().is_none() && self.grid.wires().is_empty() {
                    ui.weak("No entities");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            ui.label("Components");
                            for component in self.grid.components() {
                                let response = egui::CollapsingHeader::new(format!(
                                    "#{} {}",
                                    component.id.0,
                                    component_kind_name(&component.kind)
                                ))
                                .id_salt(("logic-grid-entity", component.id.0))
                                .show(ui, |ui| {
                                    egui::Grid::new(("logic-grid-entity-details", component.id.0))
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            ui.label("Position");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                component.position.x, component.position.y
                                            ));
                                            ui.end_row();
                                            ui.label("Orientation");
                                            ui.monospace(format!("{:?}", component.orientation));
                                            ui.end_row();
                                            ui.label("Kind");
                                            ui.monospace(format!("{:?}", component.kind));
                                            ui.end_row();
                                            ui.label("Size");
                                            match component.size() {
                                                Some(size) => {
                                                    ui.monospace(format!(
                                                        "{} x {}",
                                                        size.width, size.height
                                                    ));
                                                }
                                                None => {
                                                    ui.weak("overflow");
                                                }
                                            }
                                            ui.end_row();
                                        });
                                });

                                let body_hovered =
                                    response.body_response.is_some_and(|body| body.hovered());
                                if response.header_response.hovered() || body_hovered {
                                    hovered_entity = Some(DebugEntity::Component(component.id));
                                }
                            }

                            ui.separator();
                            ui.label("Wire segments");
                            for (index, wire) in self.grid.wires().iter().copied().enumerate() {
                                let response = egui::CollapsingHeader::new(format!(
                                    "#{index} {:?}",
                                    wire.orientation()
                                ))
                                .id_salt(("logic-grid-wire", index))
                                .show(ui, |ui| {
                                    egui::Grid::new(("logic-grid-wire-details", index))
                                        .num_columns(2)
                                        .show(ui, |ui| {
                                            ui.label("Start");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                wire.start.x, wire.start.y
                                            ));
                                            ui.end_row();
                                            ui.label("End");
                                            ui.monospace(format!(
                                                "({}, {})",
                                                wire.end.x, wire.end.y
                                            ));
                                            ui.end_row();
                                            ui.label("Orientation");
                                            ui.monospace(format!("{:?}", wire.orientation()));
                                            ui.end_row();
                                            ui.label("Length");
                                            ui.monospace(wire.length().to_string());
                                            ui.end_row();
                                            ui.label("Scale");
                                            ui.monospace(format!("{}x", wire.scale.get()));
                                            ui.end_row();
                                        });
                                });

                                let body_hovered =
                                    response.body_response.is_some_and(|body| body.hovered());
                                if response.header_response.hovered() || body_hovered {
                                    hovered_entity = Some(DebugEntity::Wire(wire));
                                }
                            }
                        });
                }
            });

        hovered_entity
    }

    pub(super) fn show_generated_graph(&self, context: &egui::Context) -> GraphHover {
        let graph = self.grid.generate_graph();
        let mut hover = GraphHover::default();

        egui::Window::new("Generated Graph")
            .default_pos([360.0, 16.0])
            .default_size([560.0, 400.0])
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Nodes");
                    ui.monospace(graph.nodes.len().to_string());
                    ui.separator();
                    ui.label("Edges");
                    ui.monospace(graph.edges.len().to_string());
                });
                ui.separator();

                if graph.nodes.is_empty() {
                    ui.weak("The generated graph is empty.");
                    return;
                }

                self.paint_generated_graph(ui, &graph, &mut hover);
            });

        hover
    }

    pub(super) fn paint_generated_graph(
        &self,
        ui: &mut egui::Ui,
        graph: &CircuitGraph,
        hover: &mut GraphHover,
    ) {
        let (positions, size) = graph_layout(graph);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        let origin = response.rect.min.to_vec2();

        for edge in &graph.edges {
            let first = positions[edge.first.0] + origin;
            let second = positions[edge.second.0] + origin;
            painter.line_segment(
                [first, second],
                egui::Stroke::new(2.0, ui.visuals().widgets.noninteractive.fg_stroke.color),
            );
        }

        for (index, node) in graph.nodes.iter().enumerate() {
            let center = positions[index] + origin;
            let rect = egui::Rect::from_center_size(center, GRAPH_NODE_SIZE);
            let (fill, title, detail) = graph_node_display(node);
            painter.rect_filled(rect, 6.0, fill);
            painter.rect_stroke(
                rect,
                6.0,
                egui::Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );
            painter.text(
                center - egui::vec2(0.0, 9.0),
                egui::Align2::CENTER_CENTER,
                format!("#{index} {title}"),
                egui::FontId::proportional(14.0),
                egui::Color32::WHITE,
            );
            painter.text(
                center + egui::vec2(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                detail,
                egui::FontId::monospace(11.0),
                egui::Color32::from_gray(225),
            );

            let node_response = ui.interact(
                rect,
                ui.id().with(("generated-graph-node", index)),
                egui::Sense::hover(),
            );
            if node_response.hovered() {
                hover.include_node(node);
            }
        }
    }

    pub(super) fn render_frame(
        &self,
        rect: egui::Rect,
        pointer_world: Option<[f32; 2]>,
        hovered_entity: Option<DebugEntity>,
        graph_hover: &GraphHover,
    ) -> RenderFrame {
        let errors = self.grid.validate();
        let snapshot = self.simulation_snapshot();
        let (
            wire_value_indices,
            connection_value_indices,
            component_value_indices,
            storage_value_indices,
            wire_values,
        ) = self.wire_value_indices(&snapshot);
        let mut bad_wires = BTreeSet::new();
        let mut bad_components = BTreeSet::new();
        for error in errors {
            match error {
                ValidationError::WireComponentIntersection { wire, component } => {
                    bad_wires.insert(wire);
                    bad_components.insert(component);
                }
                ValidationError::WireNotSnapped { wire }
                | ValidationError::WireOverflow { wire } => {
                    bad_wires.insert(wire);
                }
                ValidationError::ComponentOverflow { component }
                | ValidationError::ComponentNotSnapped { component, .. } => {
                    bad_components.insert(component);
                }
                ValidationError::ComponentOverlap { first, second } => {
                    bad_components.insert(first);
                    bad_components.insert(second);
                }
                ValidationError::InfiniteLeadBlocked { component, blocker } => {
                    bad_components.insert(component);
                    bad_components.insert(blocker);
                }
            }
        }

        let mut component_triangles = Vec::new();
        let mut draw_wires = Vec::new();
        let mut draw_rays = Vec::new();
        let mut wire_triangles = Vec::new();
        let mut value_triangles = Vec::new();
        for component in self.grid.components() {
            let ray_color = match &component.kind {
                ComponentKind::Input { .. } => DrawTriangle::INPUT_COLOR,
                ComponentKind::Output { .. } => DrawTriangle::OUTPUT_COLOR,
                _ => DrawTriangle::WIRE_COLOR,
            };
            let ray_value_index = component_value_indices
                .get(&component.id)
                .copied()
                .unwrap_or_default();
            draw_rays.extend(DrawRay::from_component(
                component,
                ray_color,
                ray_value_index,
            ));
            component_triangles.extend(DrawTriangle::component(
                component,
                bad_components.contains(&component.id),
            ));
            for connection in component.connection_slots() {
                let value_index = connection_value_indices
                    .get(&(component.id, connection))
                    .copied()
                    .unwrap_or_default();
                value_triangles.push(DrawValueTriangle::connection_marker(
                    component,
                    connection,
                    connection.scale.get() as f32 * 0.4,
                    value_index,
                ));
            }
            if let Some(value_index) = storage_value_indices.get(&component.id).copied() {
                value_triangles.extend(DrawValueTriangle::storage_state(component, value_index));
            }
            if hovered_entity == Some(DebugEntity::Component(component.id))
                || graph_hover.components.contains(&component.id)
                || self.selection.components.contains(&component.id)
            {
                component_triangles.extend(DrawTriangle::component_highlight(component));
            }
            for (_, connection) in graph_hover
                .connections
                .iter()
                .filter(|(id, _)| *id == component.id)
            {
                component_triangles
                    .extend(DrawTriangle::connection_highlight(component, *connection));
            }
        }
        // Draw a wire stub on every port the circuit graph reports as wired to
        // an actual wire net, so the wire reads as entering the component no
        // matter where along the wire the contact was made. The stub carries the
        // connected net's value index so it shows the same on/off value as the
        // wire. Ports joined directly to a touching neighbour belong to an empty
        // (wireless) net and draw nothing, so abutting components have no stub.
        let mut connection_stubs = Vec::new();
        for (index, node) in snapshot.graph.nodes.iter().enumerate() {
            let GraphNode::Connection {
                component,
                slot,
                direction,
                side,
                start,
                end,
                scale,
            } = node
            else {
                continue;
            };
            let Some(component) = self.grid.component(*component) else {
                continue;
            };
            let node_id = GraphNodeId(index);
            let net_value_index = snapshot.graph.edges.iter().find_map(|edge| {
                let other = if edge.first == node_id {
                    edge.second
                } else if edge.second == node_id {
                    edge.first
                } else {
                    return None;
                };
                let GraphNode::WireNet { wires } = &snapshot.graph.nodes[other.0] else {
                    return None;
                };
                wires
                    .first()
                    .and_then(|wire| wire_value_indices.get(wire).copied())
            });
            let Some(value_index) = net_value_index else {
                continue;
            };
            connection_stubs.push(DrawStub::for_connection(
                component,
                ConnectionSlot {
                    id: *slot,
                    direction: *direction,
                    side: *side,
                    start: *start,
                    end: *end,
                    scale: *scale,
                },
                DrawTriangle::WIRE_COLOR,
                value_index,
            ));
        }
        for wire in self.grid.wires() {
            let color = if hovered_entity == Some(DebugEntity::Wire(*wire))
                || graph_hover.wires.contains(wire)
            {
                DrawTriangle::HIGHLIGHT_COLOR
            } else if bad_wires.contains(wire) {
                DrawTriangle::ERROR_COLOR
            } else {
                DrawTriangle::WIRE_COLOR
            };
            draw_wires.push(DrawWire::new(
                *wire,
                color,
                wire_value_indices.get(wire).copied().unwrap_or_default(),
            ));
            wire_triangles.extend(DrawTriangle::wire_endpoints(*wire));
            for end in [WireEnd::Start, WireEnd::End] {
                let endpoint = WireEndpoint { wire: *wire, end };
                if hovered_entity == Some(DebugEntity::WireEndpoint(endpoint))
                    || self.selection.wire_endpoints.contains(&endpoint)
                {
                    wire_triangles.extend(DrawTriangle::wire_endpoint_highlight(
                        *wire,
                        endpoint.point(),
                    ));
                }
            }
        }

        if let Some(pointer) = pointer_world {
            let snapped = snap_point(pointer, self.active_tool_snap());
            match self.gesture.as_ref() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(*start, snapped, self.tool.scale) {
                        wire_triangles
                            .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Not,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, orientation, None)
                    {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::MergerSplitter { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::MergerSplitter,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, orientation, None)
                    {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Led { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Led,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, orientation, None)
                    {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Storage,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, orientation, None)
                    {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Input { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Input,
                    );
                    let mut tool = self.tool;
                    tool.scale = self.active_input_scale();
                    if let Some(component) = component_preview(tool, *anchor, orientation, None) {
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Output { anchor, drag_start }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Output,
                    );
                    let mut tool = self.tool;
                    tool.scale = self.active_output_scale();
                    if let Some(component) = component_preview(tool, *anchor, orientation, None) {
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Subcomponent {
                    anchor,
                    drag_start,
                    kind,
                }) => {
                    let orientation = placement_rotation(
                        *drag_start,
                        pointer,
                        self.placement_orientation,
                        ToolKind::Custom,
                    );
                    if let Some(component) =
                        component_preview(self.tool, *anchor, orientation, Some(kind))
                    {
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::MoveSelection {
                    start,
                    scale,
                    components,
                    wires,
                }) => {
                    let delta = snapped_delta(*start, pointer, *scale);
                    for (id, position) in components {
                        let Some(position) = translate_point(*position, delta) else {
                            continue;
                        };
                        let Some(original) = self.grid.component(*id) else {
                            continue;
                        };
                        let mut component = original.clone();
                        component.position = position;
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                    for wire in wires {
                        if let Some(wire) = move_selected_wire(*wire, delta) {
                            wire_triangles
                                .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                        }
                    }
                }
                Some(Gesture::SelectBox { .. }) => {}
                None => {
                    if let Some(component) = self.placement_preview_component(pointer) {
                        draw_rays.extend(DrawRay::from_component(
                            &component,
                            DrawTriangle::PREVIEW_COLOR,
                            0,
                        ));
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
            }
        }
        wire_triangles.extend(component_triangles);
        if let Some(bounds) = self.grid.bounds() {
            wire_triangles.extend(DrawTriangle::bounds(bounds, 2.0 / self.camera.zoom));
        }

        RenderFrame {
            viewport_size: [rect.width(), rect.height()],
            camera_center: self.camera.center,
            zoom: self.camera.zoom,
            grid_scale: self.tool.snap().get() as f32,
            wires: draw_wires,
            wire_values,
            rays: draw_rays,
            stubs: connection_stubs,
            value_triangles,
            triangles: wire_triangles,
        }
    }
}
pub(super) fn component_kind_name(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Not { .. } => "NOT gate",
        ComponentKind::MergerSplitter {
            input_scale,
            output_scale,
        } if input_scale <= output_scale => "Merger",
        ComponentKind::MergerSplitter { .. } => "Splitter",
        ComponentKind::Led => "LED",
        ComponentKind::Storage { .. } => "Storage",
        ComponentKind::Input { .. } => "Input",
        ComponentKind::Output { .. } => "Output",
        ComponentKind::Subcomponent { .. } => "Subcomponent",
    }
}

pub(super) fn graph_layout(graph: &CircuitGraph) -> (Vec<egui::Pos2>, egui::Vec2) {
    let mut columns = [Vec::new(), Vec::new(), Vec::new()];
    for (index, node) in graph.nodes.iter().enumerate() {
        let column = match node {
            GraphNode::Component { .. } => 0,
            GraphNode::Connection { .. } => 1,
            GraphNode::WireNet { .. } => 2,
        };
        columns[column].push(index);
    }

    let rows = columns.iter().map(Vec::len).max().unwrap_or(0).max(1);
    let row_stride = GRAPH_NODE_SIZE.y + GRAPH_ROW_GAP;
    let content_height =
        rows as f32 * GRAPH_NODE_SIZE.y + rows.saturating_sub(1) as f32 * GRAPH_ROW_GAP;
    let size = egui::vec2(
        GRAPH_MARGIN * 2.0 + GRAPH_NODE_SIZE.x * 3.0 + GRAPH_COLUMN_GAP * 2.0,
        GRAPH_MARGIN * 2.0 + content_height,
    );
    let mut positions = vec![egui::Pos2::ZERO; graph.nodes.len()];

    for (column, nodes) in columns.iter().enumerate() {
        let column_height = nodes.len() as f32 * GRAPH_NODE_SIZE.y
            + nodes.len().saturating_sub(1) as f32 * GRAPH_ROW_GAP;
        let top = GRAPH_MARGIN + (content_height - column_height) / 2.0;
        let x = GRAPH_MARGIN
            + GRAPH_NODE_SIZE.x / 2.0
            + column as f32 * (GRAPH_NODE_SIZE.x + GRAPH_COLUMN_GAP);
        for (row, node) in nodes.iter().enumerate() {
            positions[*node] =
                egui::pos2(x, top + GRAPH_NODE_SIZE.y / 2.0 + row as f32 * row_stride);
        }
    }

    (positions, size)
}

pub(super) fn graph_node_display(node: &GraphNode) -> (egui::Color32, &'static str, String) {
    match node {
        GraphNode::Component { component } => (
            egui::Color32::from_rgb(50, 105, 175),
            "Component",
            format!("component #{}", component.0),
        ),
        GraphNode::WireNet { wires } => (
            egui::Color32::from_rgb(45, 135, 85),
            "Wire net",
            format!("{} segment(s)", wires.len()),
        ),
        GraphNode::Connection {
            component,
            slot,
            direction,
            side,
            start,
            end,
            scale,
        } => (
            egui::Color32::from_rgb(155, 95, 45),
            "Connection",
            format!(
                "#{} slot {} {direction:?} {side:?} [{start}, {end}) {}x",
                component.0,
                slot.0,
                scale.get()
            ),
        ),
    }
}
