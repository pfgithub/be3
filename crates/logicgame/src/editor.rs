use std::collections::BTreeSet;

use eframe::egui::{self, PointerButton};
use logicgame::grid::{
    Component, ComponentId, ComponentKind, LogicGrid, Point, Rotation, Scale, ValidationError, Wire,
};

use crate::renderer::{DrawInstance, GridCallback, RenderFrame};

const MIN_ZOOM: f32 = 4.0;
const MAX_ZOOM: f32 = 96.0;
const DEFAULT_ZOOM: f32 = 24.0;
const WIRE_HIT_RADIUS: f32 = 7.0;
const SCALES: [u8; 7] = [1, 2, 4, 8, 16, 32, 64];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ToolKind {
    Select,
    Wire,
    Not,
}

impl ToolKind {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Wire => "Wire",
            Self::Not => "NOT gate",
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    Wire { start: Point },
    Not { anchor: Point, drag_start: [f32; 2] },
}

pub struct LogicEditor {
    grid: LogicGrid,
    tool: Tool,
    camera: Camera,
    gesture: Option<Gesture>,
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
                .interact_pointer_pos()
                .map(|position| self.camera.screen_to_world(position, response.rect));
            let frame = self.render_frame(response.rect, pointer_world);
            painter.add(eframe::egui_wgpu::Callback::new_paint_callback(
                response.rect,
                GridCallback { frame },
            ));
        });

        egui::Window::new("Tools")
            .default_pos([16.0, 16.0])
            .resizable(false)
            .show(&context, |ui| {
                for kind in [ToolKind::Select, ToolKind::Wire, ToolKind::Not] {
                    if ui
                        .selectable_label(self.tool.kind == kind, kind.label())
                        .clicked()
                    {
                        self.tool.kind = kind;
                        self.gesture = None;
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
                ui.small("Esc: cancel");
            });

        if context.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.gesture = None;
        }

        let _ = canvas;
        context.request_repaint();
    }

    fn handle_canvas_input(&mut self, response: &egui::Response) {
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
                ToolKind::Select => None,
                ToolKind::Wire => Some(Gesture::Wire { start: snapped }),
                ToolKind::Not => Some(Gesture::Not {
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
                            not_gate_position(anchor, rotation, self.tool.scale),
                            rotation,
                            ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        );
                    }
                }
                None => {}
            }
        }
    }

    fn render_frame(&self, rect: egui::Rect, pointer_world: Option<[f32; 2]>) -> RenderFrame {
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

        let mut component_instances = Vec::new();
        let mut wire_instances = Vec::new();
        for component in self.grid.components() {
            if let Some(instance) =
                DrawInstance::component(component, bad_components.contains(&component.id))
            {
                component_instances.push(instance);
            }
        }
        for wire in self.grid.wires() {
            wire_instances.push(DrawInstance::wire(
                *wire,
                if bad_wires.contains(wire) {
                    DrawInstance::ERROR_COLOR
                } else {
                    DrawInstance::WIRE_COLOR
                },
            ));
        }

        if let Some(pointer) = pointer_world {
            let snapped = snap_point(pointer, self.tool.scale);
            match self.gesture {
                Some(Gesture::Wire { start }) => {
                    if let Some(wire) = projected_wire(start, snapped, self.tool.scale) {
                        wire_instances.push(DrawInstance::wire(wire, DrawInstance::PREVIEW_COLOR));
                    }
                }
                Some(Gesture::Not { anchor, drag_start }) => {
                    if let Some(rotation) = drag_rotation(drag_start, pointer) {
                        let component = Component {
                            id: ComponentId(u64::MAX),
                            position: not_gate_position(anchor, rotation, self.tool.scale),
                            rotation,
                            kind: ComponentKind::Not {
                                scale: self.tool.scale,
                            },
                        };
                        if let Some(instance) = DrawInstance::component(&component, false) {
                            component_instances
                                .push(instance.with_color(DrawInstance::PREVIEW_COLOR));
                        }
                    }
                }
                None => {}
            }
        }
        component_instances.extend(wire_instances);

        RenderFrame {
            viewport_size: [rect.width(), rect.height()],
            camera_center: self.camera.center,
            zoom: self.camera.zoom,
            grid_scale: self.tool.scale.get() as f32,
            instances: component_instances,
        }
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

fn projected_wire(start: Point, end: Point, scale: Scale) -> Option<Wire> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let scale_value = scale.get();
    let (start, end) = if dx.abs() >= dy.abs() {
        let min_x = start.x.min(end.x);
        let max_x = start.x.max(end.x).checked_add(scale_value)?;
        (Point::new(min_x, start.y), Point::new(max_x, start.y))
    } else {
        let min_y = start.y.min(end.y);
        let max_y = start.y.max(end.y).checked_add(scale_value)?;
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

fn not_gate_position(anchor: Point, rotation: Rotation, scale: Scale) -> Point {
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
                    wire.end.x as f32,
                    wire.start.y as f32,
                    wire.start.y as f32 + scale,
                ),
                logicgame::grid::Orientation::Vertical => (
                    wire.start.x as f32,
                    wire.start.x as f32 + scale,
                    wire.start.y as f32,
                    wire.end.y as f32,
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
            projected_wire(Point::new(0, 0), Point::new(8, 3), scale(1)),
            Some(wire((0, 0), (9, 0), 1))
        );
        assert_eq!(
            projected_wire(Point::new(8, 3), Point::new(0, 0), scale(1)),
            Some(wire((0, 3), (9, 3), 1))
        );
        assert_eq!(
            projected_wire(Point::new(0, 0), Point::new(2, -9), scale(1)),
            Some(wire((0, -9), (0, 1), 1))
        );
        assert_eq!(
            projected_wire(Point::new(2, -9), Point::new(0, 0), scale(1)),
            Some(wire((2, -9), (2, 1), 1))
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
            not_gate_position(anchor, Rotation::Right, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            not_gate_position(anchor, Rotation::Down, scale(2)),
            Point::new(8, 8)
        );
        assert_eq!(
            not_gate_position(anchor, Rotation::Up, scale(2)),
            Point::new(8, 6)
        );
        assert_eq!(
            not_gate_position(anchor, Rotation::Left, scale(2)),
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
}
