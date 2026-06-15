use std::collections::BTreeSet;

use eframe::egui::{self, PointerButton};
use logicgame::grid::{
    CircuitGraph, Component, ComponentId, ComponentKind, GraphNode, LogicGrid, Point, Rotation,
    Scale, ValidationError, Wire,
};

use crate::renderer::{DrawTriangle, GridCallback, RenderFrame};

const MIN_ZOOM: f32 = 4.0;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];
const GRAPH_NODE_SIZE: egui::Vec2 = egui::vec2(150.0, 48.0);
const GRAPH_COLUMN_GAP: f32 = 70.0;
const GRAPH_ROW_GAP: f32 = 18.0;
const GRAPH_MARGIN: f32 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Select,
    Wire,
    Not,
    Led,
    Storage,
}

impl ToolKind {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Not => "NOT gate",
            Self::Led => "LED",
            Self::Storage => "Storage",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tool {
    kind: ToolKind,
    scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Camera {
    center: [f32; 2],
    zoom: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0],
            zoom: DEFAULT_ZOOM,
        }
    }
}

impl Camera {
    fn screen_to_world(self, screen: egui::Pos2, rect: egui::Rect) -> [f32; 2] {
        let relative = screen - rect.center();
        [
            self.center[0] + relative.x / self.zoom,
            self.center[1] + relative.y / self.zoom,
        ]
    }

    fn zoom_around(&mut self, screen: egui::Pos2, rect: egui::Rect, factor: f32) {
        let before = self.screen_to_world(screen, rect);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let after = self.screen_to_world(screen, rect);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Gesture {
    Wire {
        start: Point,
    },
    Not {
        anchor: Point,
        drag_start: [f32; 2],
    },
    Led {
        position: Point,
    },
    Storage {
        anchor: Point,
        drag_start: [f32; 2],
    },
    SelectBox {
        start: [f32; 2],
        additive: bool,
    },
    MoveSelection {
        start: [f32; 2],
        scale: Scale,
        components: Vec<(ComponentId, Point)>,
        wires: Vec<Wire>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DebugEntity {
    Component(ComponentId),
    Wire(Wire),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Selection {
    components: BTreeSet<ComponentId>,
    wires: BTreeSet<Wire>,
}

impl Selection {
    fn is_empty(&self) -> bool {
        self.components.is_empty() && self.wires.is_empty()
    }

    fn clear(&mut self) {
        self.components.clear();
        self.wires.clear();
    }

    fn contains(&self, entity: DebugEntity) -> bool {
        match entity {
            DebugEntity::Component(id) => self.components.contains(&id),
            DebugEntity::Wire(wire) => self.wires.contains(&wire),
        }
    }

    fn insert(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                self.components.insert(id);
            }
            DebugEntity::Wire(wire) => {
                self.wires.insert(wire);
            }
        }
    }

    fn toggle(&mut self, entity: DebugEntity) {
        match entity {
            DebugEntity::Component(id) => {
                if !self.components.remove(&id) {
                    self.components.insert(id);
                }
            }
            DebugEntity::Wire(wire) => {
                if !self.wires.remove(&wire) {
                    self.wires.insert(wire);
                }
            }
        }
    }
}

pub struct LogicEditor {
    grid: LogicGrid,
    tool: Tool,
    camera: Camera,
    gesture: Option<Gesture>,
    selection: Selection,
}

impl Default for LogicEditor {
    fn default() -> Self {
        Self {
            grid: LogicGrid::new(),
            tool: Tool {
                kind: ToolKind::Select,
                scale: Scale::ONE,
            },
            camera: Camera::default(),
            gesture: None,
            selection: Selection::default(),
        }
    }
}

impl LogicEditor {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let context = ui.ctx().clone();
        let canvas = egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
            self.handle_canvas_input(&response);

            let pointer_world = response
                .hovered()
                .then(|| context.pointer_hover_pos())
                .flatten()
                .map(|position| self.camera.screen_to_world(position, response.rect));
            (response, painter, pointer_world)
        });

        egui::Window::new("Tools")
            .default_pos([16.0, 16.0])
            .resizable(false)
            .show(&context, |ui| {
                for kind in [
                    ToolKind::Select,
                    ToolKind::Wire,
                    ToolKind::Not,
                    ToolKind::Led,
                    ToolKind::Storage,
                ] {
                    if ui
                        .selectable_label(self.tool.kind == kind, kind.label())
                        .clicked()
                    {
                        self.tool.kind = kind;
                        self.gesture = None;
                        if kind != ToolKind::Select {
                            self.selection.clear();
                        }
                    }
                }

                ui.separator();
                ui.label("Scale");
                egui::ComboBox::from_id_salt("logic-tool-scale")
                    .selected_text(format!("{}x", self.tool.scale.get()))
                    .show_ui(ui, |ui| {
                        for value in SCALES {
                            let scale = Scale::new(value).expect("tool scale is valid");
                            ui.selectable_value(&mut self.tool.scale, scale, format!("{value}x"));
                        }
                    });
                ui.separator();
                ui.small("Middle drag: pan");
                ui.small("Wheel: zoom");
                ui.small("Shift: add/remove selection");
                ui.small("Delete: remove selection");
                ui.small("Esc: cancel");
            });

        let hovered_square = canvas
            .inner
            .2
            .map(|pointer| snap_point(pointer, self.tool.scale));
        let hovered_entity = self.show_grid_debugger(&context, hovered_square);
        let hovered_graph_wires = self.show_generated_graph(&context);
        let frame = self.render_frame(
            canvas.inner.0.rect,
            canvas.inner.2,
            hovered_entity,
            &hovered_graph_wires,
        );
        canvas
            .inner
            .1
            .add(eframe::egui_wgpu::Callback::new_paint_callback(
                canvas.inner.0.rect,
                GridCallback { frame },
            ));
        if let (Some(pointer), Some(Gesture::SelectBox { start, .. })) =
            (canvas.inner.2, self.gesture.as_ref())
        {
            let start = world_to_screen(*start, self.camera, canvas.inner.0.rect);
            let end = world_to_screen(pointer, self.camera, canvas.inner.0.rect);
            let selection_rect = egui::Rect::from_two_pos(start, end);
            canvas.inner.1.rect_filled(
                selection_rect,
                0.0,
                egui::Color32::from_rgba_unmultiplied(66, 153, 225, 28),
            );
            canvas.inner.1.rect_stroke(
                selection_rect,
                0.0,
                egui::Stroke::new(1.0, egui::Color32::from_rgb(90, 180, 255)),
                egui::StrokeKind::Inside,
            );
        }

        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.gesture = None;
        }

        context.request_repaint();
    }

    fn show_grid_debugger(
        &self,
        context: &egui::Context,
        hovered_square: Option<Point>,
    ) -> Option<DebugEntity> {
        let errors = self.grid.validate();
        let mut hovered_entity = None;

        egui::Window::new("Grid Debugger")
            .default_pos([700.0, 16.0])
            .default_width(240.0)
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
                                            ui.label("Rotation");
                                            ui.monospace(format!("{:?}", component.rotation));
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

    fn show_generated_graph(&self, context: &egui::Context) -> BTreeSet<Wire> {
        let graph = self.grid.generate_graph();
        let mut hovered_wires = BTreeSet::new();

        egui::Window::new("Generated Graph")
            .default_pos([360.0, 16.0])
            .default_size([560.0, 400.0])
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

                egui::ScrollArea::both().show(ui, |ui| {
                    self.paint_generated_graph(ui, &graph, &mut hovered_wires);
                });
            });

        hovered_wires
    }

    fn paint_generated_graph(
        &self,
        ui: &mut egui::Ui,
        graph: &CircuitGraph,
        hovered_wires: &mut BTreeSet<Wire>,
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
                if let GraphNode::WireNet { wires } = node {
                    hovered_wires.extend(wires.iter().copied());
                }
            }
        }
    }

    fn handle_canvas_input(&mut self, response: &egui::Response) {
        if self.tool.kind == ToolKind::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
        {
            self.delete_selection();
        }

        let Some(pointer) = response.interact_pointer_pos() else {
            return;
        };

        if response.hovered() {
            let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera
                    .zoom_around(pointer, response.rect, (scroll * 0.002).exp());
            }
        }

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
        let snapped = snap_point(world, self.tool.scale);

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
                ToolKind::Led => Some(Gesture::Led { position: snapped }),
                ToolKind::Storage => Some(Gesture::Storage {
                    anchor: snapped,
                    drag_start: world,
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
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        self.grid.add_component(
                            oriented_component_position(anchor, rotation, self.tool.scale),
                            rotation,
                            ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        );
                    }
                }
                Some(Gesture::Led { position }) => {
                    self.grid
                        .add_component(position, Rotation::Up, ComponentKind::Led);
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, world) {
                        self.grid.add_component(
                            oriented_component_position(anchor, rotation, self.tool.scale),
                            rotation,
                            ComponentKind::Storage {
                                scale: self.tool.scale,
                            },
                        );
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

    fn entity_at(&self, point: [f32; 2]) -> Option<DebugEntity> {
        self.grid
            .components()
            .filter(|component| component_contains(component, point))
            .map(|component| DebugEntity::Component(component.id))
            .max_by_key(|entity| match entity {
                DebugEntity::Component(id) => *id,
                DebugEntity::Wire(_) => unreachable!(),
            })
            .or_else(|| {
                nearest_wire(self.grid.wires(), point, WIRE_HIT_RADIUS / self.camera.zoom)
                    .map(DebugEntity::Wire)
            })
    }

    fn move_gesture(&self, start: [f32; 2]) -> Option<Gesture> {
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
        let wires: Vec<_> = self.selection.wires.iter().copied().collect();
        let scale = components
            .iter()
            .filter_map(|(id, _)| self.grid.component(*id))
            .map(|component| component.kind.snap())
            .chain(wires.iter().map(|wire| wire.scale))
            .max()
            .unwrap_or(Scale::ONE);
        Some(Gesture::MoveSelection {
            start,
            scale,
            components,
            wires,
        })
    }

    fn select_in_rect(&mut self, start: [f32; 2], end: [f32; 2]) {
        let rect = WorldRect::from_points(start, end);
        self.selection.components.extend(
            self.grid
                .components()
                .filter(|component| component_intersects(component, rect))
                .map(|component| component.id),
        );
        self.selection.wires.extend(
            self.grid
                .wires()
                .iter()
                .copied()
                .filter(|wire| wire_intersects(*wire, rect)),
        );
    }

    fn apply_move(&mut self, components: &[(ComponentId, Point)], wires: &[Wire], delta: Point) {
        if delta == Point::new(0, 0) {
            return;
        }
        let moved_components: Option<Vec<_>> = components
            .iter()
            .map(|(id, position)| translate_point(*position, delta).map(|point| (*id, point)))
            .collect();
        let moved_wires: Option<Vec<_>> = wires
            .iter()
            .map(|wire| translate_wire(*wire, delta))
            .collect();
        let (Some(moved_components), Some(moved_wires)) = (moved_components, moved_wires) else {
            return;
        };

        for wire in wires {
            self.grid.remove_wire(*wire);
        }
        for (id, position) in moved_components {
            self.grid.set_component_position(id, position);
        }
        for wire in &moved_wires {
            self.grid.add_wire(*wire);
        }

        self.selection.wires = self
            .grid
            .wires()
            .iter()
            .copied()
            .filter(|wire| moved_wires.iter().any(|moved| wires_overlap(*wire, *moved)))
            .collect();
    }

    fn delete_selection(&mut self) {
        for id in std::mem::take(&mut self.selection.components) {
            self.grid.remove_component(id);
        }
        for wire in std::mem::take(&mut self.selection.wires) {
            self.grid.remove_wire(wire);
        }
        self.gesture = None;
    }

    fn render_frame(
        &self,
        rect: egui::Rect,
        pointer_world: Option<[f32; 2]>,
        hovered_entity: Option<DebugEntity>,
        hovered_graph_wires: &BTreeSet<Wire>,
    ) -> RenderFrame {
        let errors = self.grid.validate();
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
            }
        }

        let mut component_triangles = Vec::new();
        let mut wire_triangles = Vec::new();
        for component in self.grid.components() {
            if hovered_entity == Some(DebugEntity::Component(component.id))
                || self.selection.components.contains(&component.id)
            {
                component_triangles.extend(DrawTriangle::component_highlight(component));
            }
            component_triangles.extend(DrawTriangle::component(
                component,
                bad_components.contains(&component.id),
            ));
        }
        for wire in self.grid.wires() {
            wire_triangles.extend(DrawTriangle::wire(
                *wire,
                if hovered_entity == Some(DebugEntity::Wire(*wire))
                    || hovered_graph_wires.contains(wire)
                    || self.selection.wires.contains(wire)
                {
                    DrawTriangle::HIGHLIGHT_COLOR
                } else if bad_wires.contains(wire) {
                    DrawTriangle::ERROR_COLOR
                } else {
                    DrawTriangle::WIRE_COLOR
                },
            ));
        }

        if let Some(pointer) = pointer_world {
            let snapped = snap_point(pointer, self.tool.scale);
            match self.gesture.as_ref() {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(*start, snapped, self.tool.scale) {
                        wire_triangles
                            .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: oriented_component_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                            ),
                            rotation,
                            kind: ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        };
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                }
                Some(Gesture::Led { position }) => {
                    let component = Component {
                        id: ComponentId(u64::MAX),
                        position: *position,
                        rotation: Rotation::Up,
                        kind: ComponentKind::Led,
                    };
                    component_triangles.extend(
                        DrawTriangle::component(&component, false)
                            .into_iter()
                            .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                    );
                }
                Some(Gesture::Storage { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(*drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: oriented_component_position(
                                *anchor,
                                rotation,
                                self.tool.scale,
                            ),
                            rotation,
                            kind: ComponentKind::Storage {
                                scale: self.tool.scale,
                            },
                        };
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
                        component_triangles.extend(
                            DrawTriangle::component(&component, false)
                                .into_iter()
                                .map(|triangle| triangle.with_color(DrawTriangle::PREVIEW_COLOR)),
                        );
                    }
                    for wire in wires {
                        if let Some(wire) = translate_wire(*wire, delta) {
                            wire_triangles
                                .extend(DrawTriangle::wire(wire, DrawTriangle::PREVIEW_COLOR));
                        }
                    }
                }
                Some(Gesture::SelectBox { .. }) => {}
                None => {}
            }
        }
        component_triangles.extend(wire_triangles);

        RenderFrame {
            viewport_size: [rect.width(), rect.height()],
            camera_center: self.camera.center,
            zoom: self.camera.zoom,
            grid_scale: self.tool.scale.get() as f32,
            triangles: component_triangles,
        }
    }
}

fn component_kind_name(kind: &ComponentKind) -> &'static str {
    match kind {
        ComponentKind::Not { .. } => "NOT gate",
        ComponentKind::Led => "LED",
        ComponentKind::Storage { .. } => "Storage",
        ComponentKind::Subcomponent { .. } => "Subcomponent",
    }
}

fn snap_coordinate(value: f32, scale: Scale) -> i64 {
    let scale = scale.get();
    (value / scale as f32).floor() as i64 * scale
}

fn snap_point(point: [f32; 2], scale: Scale) -> Point {
    Point::new(
        snap_coordinate(point[0], scale),
        snap_coordinate(point[1], scale),
    )
}

fn snapped_delta(start: [f32; 2], end: [f32; 2], scale: Scale) -> Point {
    let scale = scale.get() as f32;
    Point::new(
        ((end[0] - start[0]) / scale).round() as i64 * scale as i64,
        ((end[1] - start[1]) / scale).round() as i64 * scale as i64,
    )
}

fn translate_point(point: Point, delta: Point) -> Option<Point> {
    Some(Point::new(
        point.x.checked_add(delta.x)?,
        point.y.checked_add(delta.y)?,
    ))
}

fn translate_wire(wire: Wire, delta: Point) -> Option<Wire> {
    Wire::new(
        translate_point(wire.start, delta)?,
        translate_point(wire.end, delta)?,
        wire.scale,
    )
    .ok()
}

fn world_to_screen(world: [f32; 2], camera: Camera, rect: egui::Rect) -> egui::Pos2 {
    rect.center()
        + egui::vec2(
            (world[0] - camera.center[0]) * camera.zoom,
            (world[1] - camera.center[1]) * camera.zoom,
        )
}

#[derive(Clone, Copy)]
struct WorldRect {
    min: [f32; 2],
    max: [f32; 2],
}

impl WorldRect {
    fn from_points(first: [f32; 2], second: [f32; 2]) -> Self {
        Self {
            min: [first[0].min(second[0]), first[1].min(second[1])],
            max: [first[0].max(second[0]), first[1].max(second[1])],
        }
    }

    fn intersects(self, min: [f32; 2], max: [f32; 2]) -> bool {
        self.min[0] <= max[0]
            && min[0] <= self.max[0]
            && self.min[1] <= max[1]
            && min[1] <= self.max[1]
    }
}

fn component_contains(component: &Component, point: [f32; 2]) -> bool {
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

fn component_intersects(component: &Component, rect: WorldRect) -> bool {
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

fn wire_bounds(wire: Wire) -> ([f32; 2], [f32; 2]) {
    let scale = wire.scale.get() as f32;
    match wire.orientation() {
        logicgame::grid::Orientation::Horizontal => (
            [wire.start.x as f32, wire.start.y as f32],
            [wire.end.x as f32 + scale, wire.start.y as f32 + scale],
        ),
        logicgame::grid::Orientation::Vertical => (
            [wire.start.x as f32, wire.start.y as f32],
            [wire.start.x as f32 + scale, wire.end.y as f32 + scale],
        ),
    }
}

fn wire_intersects(wire: Wire, rect: WorldRect) -> bool {
    let (min, max) = wire_bounds(wire);
    rect.intersects(min, max)
}

fn wires_overlap(first: Wire, second: Wire) -> bool {
    first.scale == second.scale
        && first.orientation() == second.orientation()
        && match first.orientation() {
            logicgame::grid::Orientation::Horizontal => {
                first.start.y == second.start.y
                    && first.start.x <= second.end.x
                    && second.start.x <= first.end.x
            }
            logicgame::grid::Orientation::Vertical => {
                first.start.x == second.start.x
                    && first.start.y <= second.end.y
                    && second.start.y <= first.end.y
            }
        }
}

fn projected_wire(start: Point, end: Point, scale: Scale) -> Option<Wire> {
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

fn drag_rotation(start: [f32; 2], pointer: [f32; 2]) -> Option<Rotation> {
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

fn oriented_component_position(anchor: Point, rotation: Rotation, scale: Scale) -> Point {
    let scale = scale.get();
    match rotation {
        Rotation::Up => Point::new(anchor.x, anchor.y - scale),
        Rotation::Right | Rotation::Down => anchor,
        Rotation::Left => Point::new(anchor.x - scale, anchor.y),
    }
}

fn nearest_wire(wires: &[Wire], point: [f32; 2], radius: f32) -> Option<Wire> {
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

fn graph_layout(graph: &CircuitGraph) -> (Vec<egui::Pos2>, egui::Vec2) {
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

fn graph_node_display(node: &GraphNode) -> (egui::Color32, &'static str, String) {
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
            side,
            start,
            end,
        } => (
            egui::Color32::from_rgb(155, 95, 45),
            "Connection",
            format!("#{} {side:?} [{start}, {end})", component.0),
        ),
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
    fn screen_world_round_trip_and_cursor_zoom_are_stable() {
        let rect = egui::Rect::from_min_size(egui::pos2(20.0, 40.0), egui::vec2(800.0, 600.0));
        let mut camera = Camera {
            center: [12.0, -8.0],
            zoom: 20.0,
        };
        let cursor = egui::pos2(187.0, 249.0);
        let before = camera.screen_to_world(cursor, rect);
        camera.zoom_around(cursor, rect, 2.0);
        let after = camera.screen_to_world(cursor, rect);
        assert!((before[0] - after[0]).abs() < 0.0001);
        assert!((before[1] - after[1]).abs() < 0.0001);
    }

    #[test]
    fn snapping_selects_the_containing_grid_cell() {
        assert_eq!(snap_point([0.0, 0.0], scale(4)), Point::new(0, 0));
        assert_eq!(snap_point([3.99, 3.99], scale(4)), Point::new(0, 0));
        assert_eq!(snap_point([4.0, 4.0], scale(4)), Point::new(4, 4));
        assert_eq!(snap_point([-0.01, -0.01], scale(4)), Point::new(-4, -4));
        assert_eq!(snap_point([-4.0, -4.0], scale(4)), Point::new(-4, -4));
    }

    #[test]
    fn wire_projection_uses_the_dominant_axis() {
        assert_eq!(
            projected_wire(Point::new(1, 1), Point::new(2, 1), scale(1)),
            Some(wire((1, 1), (2, 1), 1))
        );
        assert_eq!(
            projected_wire(Point::new(0, 0), Point::new(8, 3), scale(1)),
            Some(wire((0, 0), (8, 0), 1))
        );
        assert_eq!(
            projected_wire(Point::new(8, 3), Point::new(0, 0), scale(1)),
            Some(wire((0, 3), (8, 3), 1))
        );
        assert_eq!(
            projected_wire(Point::new(0, 0), Point::new(2, -9), scale(1)),
            Some(wire((0, -9), (0, 0), 1))
        );
        assert_eq!(
            projected_wire(Point::new(2, -9), Point::new(0, 0), scale(1)),
            Some(wire((2, -9), (2, 0), 1))
        );
    }

    #[test]
    fn gate_drag_maps_to_rotation_and_input_anchor() {
        let anchor = Point::new(8, 8);
        assert_eq!(
            drag_rotation([9.5, 9.5], [13.0, 9.0]),
            Some(Rotation::Right)
        );
        assert_eq!(drag_rotation([9.5, 9.5], [8.5, 9.5]), Some(Rotation::Left));
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 8.5]), Some(Rotation::Up));
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 10.5]), Some(Rotation::Down));
        assert_eq!(
            drag_rotation([9.5, 9.5], [9.500_001, 9.5]),
            Some(Rotation::Right)
        );
        assert_eq!(drag_rotation([9.5, 9.5], [9.5, 9.5]), None);
        assert_eq!(
            oriented_component_position(anchor, Rotation::Right, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Down, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Up, scale(2)),
            Point::new(8, 6)
        );
        assert_eq!(
            oriented_component_position(anchor, Rotation::Left, scale(2)),
            Point::new(6, 8)
        );
    }

    #[test]
    fn wire_hit_testing_selects_nearest_segment() {
        let horizontal = wire((0, 0), (10, 0), 1);
        let vertical = wire((5, -5), (5, 5), 1);
        assert_eq!(
            nearest_wire(&[vertical, horizontal], [2.0, 0.5], 0.6),
            Some(horizontal)
        );
        assert_eq!(nearest_wire(&[horizontal], [2.0, 3.0], 0.5), None);
    }

    #[test]
    fn selection_movement_snaps_to_its_largest_scale() {
        assert_eq!(
            snapped_delta([1.0, 1.0], [12.9, -3.1], scale(8)),
            Point::new(8, -8)
        );
        assert_eq!(
            snapped_delta([1.0, 1.0], [13.1, 5.1], scale(8)),
            Point::new(16, 8)
        );
    }

    #[test]
    fn mixed_selection_moves_and_deletes_components_and_wires() {
        let mut editor = LogicEditor::default();
        let component = editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let original_wire = wire((0, 8), (16, 8), 8);
        editor.grid.add_wire(original_wire);
        editor.selection.components.insert(component);
        editor.selection.wires.insert(original_wire);

        let gesture = editor.move_gesture([0.0, 0.0]).unwrap();
        let Gesture::MoveSelection {
            scale: snap_scale,
            components,
            wires,
            ..
        } = gesture
        else {
            panic!("expected a move gesture");
        };
        assert_eq!(snap_scale, scale(8));

        editor.apply_move(&components, &wires, Point::new(8, -8));
        assert_eq!(
            editor.grid.component(component).unwrap().position,
            Point::new(8, -8)
        );
        assert_eq!(editor.grid.wires(), &[wire((8, 0), (24, 0), 8)]);
        assert!(editor.grid.validate().is_empty());

        editor.delete_selection();
        assert!(editor.grid.component(component).is_none());
        assert!(editor.grid.wires().is_empty());
        assert!(editor.selection.is_empty());
    }

    #[test]
    fn box_selection_finds_intersecting_components_and_wire_segments() {
        let mut editor = LogicEditor::default();
        let inside = editor.grid.add_component(
            Point::new(0, 0),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let outside = editor.grid.add_component(
            Point::new(20, 20),
            Rotation::Right,
            ComponentKind::Not { scale: scale(2) },
        );
        let selected_wire = wire((0, 6), (8, 6), 2);
        editor.grid.add_wire(selected_wire);

        editor.select_in_rect([-1.0, -1.0], [9.0, 9.0]);

        assert!(editor.selection.components.contains(&inside));
        assert!(!editor.selection.components.contains(&outside));
        assert!(editor.selection.wires.contains(&selected_wire));
    }
}
