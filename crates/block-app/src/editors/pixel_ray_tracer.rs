mod raytracer;

use std::{
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorViewport, EditorAction,
    EditorRegistration,
};
use block::Block;
use block_client::{
    blocks::pixel_ray_tracer::{
        PixelRayTracer, PixelRayTracerOperation, PixelUpdate, Point, RayEntity, RaySettings,
        PIXEL_RAY_TRACER_PALETTE, PIXEL_RAY_TRACER_SIZE,
    },
    BlockHandle,
};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use egui_material_icons::icons::ICON_FLARE;

use crate::performance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LightingCacheKey {
    block_revision: u64,
    preview_revision: u64,
}

type LightingJobResult = (LightingCacheKey, Vec<[u8; 4]>, Duration);
type RayJobResult = (
    Point,
    Vec<[u8; 4]>,
    Vec<Point>,
    u64,
    RaySettings,
    bool,
    Duration,
);

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: PixelRayTracer::TYPE_ID,
        display_name: "Pixel Ray Tracer",
        icon: ICON_FLARE,
        create: Some(|client| {
            Box::new(PixelRayTracerEditor::new(
                client.create_block(PixelRayTracer::new()),
            ))
        }),
        open: |client, id| {
            Box::new(PixelRayTracerEditor::new(
                client.get_block::<PixelRayTracer>(id),
            ))
        },
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Pencil,
    PixelLine,
    Select,
    Surface,
    Water,
    Light,
    RayTrace,
}

impl Tool {
    const ALL: [Self; 7] = [
        Self::Pencil,
        Self::PixelLine,
        Self::Select,
        Self::Surface,
        Self::Water,
        Self::Light,
        Self::RayTrace,
    ];
    const fn label(self) -> &'static str {
        match self {
            Self::Pencil => "Pencil",
            Self::PixelLine => "Pixel line",
            Self::Select => "Select entity",
            Self::Surface => "Surface",
            Self::Water => "Water",
            Self::Light => "Light",
            Self::RayTrace => "Ray trace",
        }
    }
}

/// A rendered single-ray debug overlay: the ray's origin, its pixels, the debug
/// points along it, and the revision and settings it was rendered for.
type RayOverlay = (Point, Vec<[u8; 4]>, Vec<Point>, u64, RaySettings, bool);

#[derive(Clone)]
enum ActiveInteraction {
    Pixels {
        start: (u16, u16),
        current: (u16, u16),
        path: Vec<(u16, u16)>,
    },
    Entity {
        start: Point,
        current: Point,
    },
    Endpoint {
        id: u64,
        start: bool,
        current: Point,
    },
    Light {
        id: u64,
        current: Point,
    },
}

pub(super) struct PixelRayTracerEditor {
    block: BlockHandle<PixelRayTracer>,
    tool: Tool,
    color_index: u8,
    selected_entity: Option<u64>,
    interaction: Option<ActiveInteraction>,
    interaction_revision: u64,
    new_light_intensity: f32,
    new_surface_roughness: f32,
    new_surface_metalness: f32,
    new_surface_transmission: f32,
    new_surface_ior: f32,
    debug_overlay: bool,
    texture: Option<TextureHandle>,
    texture_revision: Option<LightingCacheKey>,
    lighting_job: Option<Receiver<LightingJobResult>>,
    rendered: Vec<[u8; 4]>,
    ray_overlay: Option<RayOverlay>,
    ray_texture: Option<TextureHandle>,
    ray_job: Option<Receiver<RayJobResult>>,
}

impl PixelRayTracerEditor {
    fn new(block: BlockHandle<PixelRayTracer>) -> Self {
        Self {
            block,
            tool: Tool::Pencil,
            color_index: 0,
            selected_entity: None,
            interaction: None,
            interaction_revision: 0,
            new_light_intensity: 2.0,
            new_surface_roughness: 0.0,
            new_surface_metalness: 0.0,
            new_surface_transmission: 0.0,
            new_surface_ior: 1.5,
            debug_overlay: false,
            texture: None,
            texture_revision: None,
            lighting_job: None,
            rendered: Vec::new(),
            ray_overlay: None,
            ray_texture: None,
            ray_job: None,
        }
    }

    fn performance_group(&self) -> String {
        format!("Pixel ray tracer ({})", self.block.id())
    }

    fn ensure_texture(&mut self, context: &egui::Context) -> bool {
        let _performance_group = performance::GroupGuard::new(self.performance_group());
        let Some(scene) = self.block.read() else {
            return false;
        };
        let revision = LightingCacheKey {
            block_revision: scene.lighting_revision(),
            preview_revision: self
                .dragged_entity(scene.entities())
                .map_or(0, |_| self.interaction_revision),
        };
        let completed = self
            .lighting_job
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(TryRecvError::Disconnected) => Some(Err(())),
                Err(TryRecvError::Empty) => None,
            });
        if let Some(completed) = completed {
            self.lighting_job = None;
            if let Ok((completed_revision, pixels, duration)) = completed {
                performance::record_duration("Lighting trace", duration);
                self.rendered = pixels;
                self.ray_overlay = None;
                self.ray_texture = None;
                let image = color_image(&self.rendered);
                if let Some(texture) = &mut self.texture {
                    texture.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.texture = Some(context.load_texture(
                        format!("pixel-ray-tracer-{}", self.block.id()),
                        image,
                        egui::TextureOptions::NEAREST,
                    ));
                }
                self.texture_revision = Some(completed_revision);
            }
        }
        if self.texture_revision == Some(revision) {
            performance::increment("Lighting cache hits");
            return true;
        }
        if self.lighting_job.is_none() {
            let pixels = scene.pixels().to_vec();
            let draft = self.dragged_entity(scene.entities());
            let entities = scene
                .entities()
                .iter()
                .map(|entity| {
                    draft
                        .as_ref()
                        .filter(|draft| draft.id() == entity.id())
                        .unwrap_or(entity)
                        .clone()
                })
                .collect::<Vec<_>>();
            let settings = scene.lighting_ray_settings();
            let repaint = context.clone();
            let (sender, receiver) = mpsc::channel();
            thread::Builder::new()
                .name("pixel-ray-tracer-lighting".into())
                .spawn(move || {
                    let start = Instant::now();
                    let result = raytracer::trace_lighting(&pixels, &entities, settings);
                    let _ = sender.send((revision, result, start.elapsed()));
                    repaint.request_repaint();
                })
                .expect("failed to start pixel ray tracer lighting job");
            self.lighting_job = Some(receiver);
            performance::increment("Lighting cache misses");
        }
        drop(scene);
        self.texture.is_some()
    }

    fn paint_block_texture(&self, context: BlockRenderContext<'_>) {
        let Some(texture) = &self.texture else { return };
        let tint =
            Color32::from_white_alpha((context.opacity.clamp(0.0, 1.0) * 255.0).round() as u8);
        let mut mesh = egui::Mesh::with_texture(texture.id());
        for (position, uv) in context.corners.into_iter().zip([
            Pos2::ZERO,
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
        ]) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos: position,
                uv,
                color: tint,
            });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        context.painter.add(mesh);
    }

    fn tools(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tools");
        for tool in Tool::ALL {
            if ui
                .add_sized(
                    [112.0, 25.0],
                    egui::Button::new(tool.label()).selected(self.tool == tool),
                )
                .clicked()
            {
                self.tool = tool;
                self.interaction = None;
                if tool != Tool::Select {
                    self.selected_entity = None;
                }
            }
        }
        ui.separator();
        ui.weak("Wheel: zoom\nMiddle drag: pan\nDelete: remove entity");
    }

    fn properties(&mut self, ui: &mut egui::Ui) {
        ui.heading("Palette");
        let mut chosen = None;
        egui::Grid::new(("ray-palette", self.block.id()))
            .num_columns(4)
            .show(ui, |ui| {
                for (index, rgb) in PIXEL_RAY_TRACER_PALETTE.iter().enumerate() {
                    let color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
                    let button = egui::Button::new("  ").fill(color).stroke(
                        if usize::from(self.color_index) == index {
                            Stroke::new(2.0, ui.visuals().selection.stroke.color)
                        } else {
                            Stroke::new(1.0, Color32::GRAY)
                        },
                    );
                    if ui
                        .add_sized([28.0, 28.0], button)
                        .on_hover_text(format!("Color {}", index + 1))
                        .clicked()
                    {
                        chosen = Some(index as u8);
                    }
                    if index % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        if let Some(index) = chosen {
            self.choose_color(index);
        }

        let selected = self.block.read().and_then(|scene| {
            self.selected_entity.and_then(|id| {
                scene
                    .entities()
                    .iter()
                    .find(|entity| entity.id() == id)
                    .cloned()
            })
        });
        if let Some(mut entity) = selected {
            ui.separator();
            ui.heading("Selected entity");
            let mut changed = false;
            match &mut entity {
                RayEntity::Surface {
                    color_index,
                    roughness,
                    metalness,
                    transmission,
                    refractive_index,
                    ..
                } => {
                    ui.label("Surface");
                    changed |= ui
                        .add(egui::Slider::new(roughness, 0.0..=1.0).text("Roughness"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(metalness, 0.0..=1.0).text("Metalness"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(transmission, 0.0..=1.0).text("Transmission"))
                        .changed();
                    changed |= ui
                        .add(egui::Slider::new(refractive_index, 1.0..=3.0).text("IOR"))
                        .changed();
                    if let Some(index) = chosen {
                        *color_index = index;
                        changed = true;
                    }
                }
                RayEntity::Light {
                    color_index,
                    intensity,
                    ..
                } => {
                    ui.label("Light");
                    changed |= ui
                        .add(egui::Slider::new(intensity, 0.1..=8.0).text("Intensity"))
                        .changed();
                    if let Some(index) = chosen {
                        *color_index = index;
                        changed = true;
                    }
                }
                RayEntity::Water { .. } => {
                    ui.label("Water");
                }
            }
            if changed {
                self.block
                    .operate(PixelRayTracerOperation::UpdateEntity { entity });
            }
            if ui.button("Delete entity").clicked() {
                self.delete_selected();
            }
        } else {
            ui.separator();
            match self.tool {
                Tool::Light => {
                    ui.heading("New light");
                    ui.add(
                        egui::Slider::new(&mut self.new_light_intensity, 0.1..=8.0)
                            .text("Intensity"),
                    );
                }
                Tool::Surface => {
                    ui.heading("New surface");
                    ui.add(
                        egui::Slider::new(&mut self.new_surface_roughness, 0.0..=1.0)
                            .text("Roughness"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.new_surface_metalness, 0.0..=1.0)
                            .text("Metalness"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.new_surface_transmission, 0.0..=1.0)
                            .text("Transmission"),
                    );
                    ui.add(egui::Slider::new(&mut self.new_surface_ior, 1.0..=3.0).text("IOR"));
                }
                _ => {}
            }
        }

        if self.tool == Tool::RayTrace {
            ui.separator();
            ui.heading("View rays");
            if let Some(scene) = self.block.read() {
                let mut settings = scene.view_ray_settings();
                drop(scene);
                if ray_settings_ui(ui, &mut settings) {
                    self.block
                        .operate(PixelRayTracerOperation::SetViewRaySettings { settings });
                    self.ray_overlay = None;
                }
            }
            ui.checkbox(&mut self.debug_overlay, "Debug positions");
        }
        ui.separator();
        ui.heading("Lighting rays");
        if let Some(scene) = self.block.read() {
            let mut settings = scene.lighting_ray_settings();
            drop(scene);
            if ray_settings_ui(ui, &mut settings) {
                self.block
                    .operate(PixelRayTracerOperation::SetLightingRaySettings { settings });
            }
        }
    }

    fn choose_color(&mut self, index: u8) {
        self.color_index = index;
        let entity = self.block.read().and_then(|scene| {
            self.selected_entity.and_then(|id| {
                scene
                    .entities()
                    .iter()
                    .find(|entity| entity.id() == id)
                    .cloned()
            })
        });
        if let Some(mut entity) = entity {
            match &mut entity {
                RayEntity::Surface { color_index, .. } | RayEntity::Light { color_index, .. } => {
                    *color_index = index;
                    self.block
                        .operate(PixelRayTracerOperation::UpdateEntity { entity });
                }
                RayEntity::Water { .. } => {}
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected_entity.take() {
            self.block
                .operate(PixelRayTracerOperation::DeleteEntity { id });
        }
    }

    fn canvas(&mut self, ui: &mut egui::Ui, viewport_controls: &mut DirectEditorViewport) {
        if !self.ensure_texture(ui.ctx()) {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        let available = ui.available_size().max(Vec2::splat(1.0));
        let (viewport, response) = ui.allocate_exact_size(available, Sense::click_and_drag());
        let painter = ui.painter_at(viewport);
        painter.rect_filled(viewport, 0.0, ui.visuals().extreme_bg_color);
        let canvas_size = Vec2::splat(viewport.width().min(viewport.height()));
        let canvas = Rect::from_center_size(viewport.center(), canvas_size);
        if let Some(texture) = &self.texture {
            painter.image(
                texture.id(),
                canvas,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        let panning = response.ctx.input(|input| {
            input.pointer.button_down(PointerButton::Middle)
                || (input.key_down(egui::Key::Space)
                    && input.pointer.button_down(PointerButton::Primary))
        });
        if panning && response.hovered() {
            viewport_controls.pan(response.ctx.input(|input| input.pointer.delta()));
            self.interaction = None;
        }
        if response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    viewport_controls.change_zoom((scroll * 0.002).exp(), Some(pointer));
                }
            }
        }
        let pointer = response
            .ctx
            .pointer_hover_pos()
            .filter(|position| canvas.contains(*position));
        let world = pointer.map(|position| screen_to_world(position, canvas));
        if response.hovered() && !panning {
            response.ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        let (pressed, down, released, delete) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_released(PointerButton::Primary),
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace),
            )
        });
        if delete {
            self.delete_selected();
        }
        if pressed && response.hovered() && response.is_pointer_button_down_on() && !panning {
            if let Some(point) = world {
                self.begin_interaction(point);
            }
        }
        if down {
            if let (Some(point), Some(interaction)) = (world, &mut self.interaction) {
                match interaction {
                    ActiveInteraction::Pixels { current, path, .. } => {
                        let next = pixel_at(point, true).unwrap_or(*current);
                        if self.tool == Tool::Pencil && next != *current {
                            path.extend(raster_line(*current, next).into_iter().skip(1));
                        }
                        *current = next;
                    }
                    ActiveInteraction::Entity { current, .. } => {
                        *current = snap(point);
                    }
                    ActiveInteraction::Endpoint { current, .. }
                    | ActiveInteraction::Light { current, .. } => {
                        let next = snap(point);
                        if *current != next {
                            *current = next;
                            self.interaction_revision = self.interaction_revision.wrapping_add(1);
                        }
                    }
                }
            }
        }
        if released {
            self.finish_interaction();
        }
        self.draw_entities(&painter, canvas);
        self.draw_interaction(&painter, canvas);
        if self.tool == Tool::RayTrace {
            if let Some(origin) = world {
                self.draw_ray_trace(&painter, canvas, origin);
            }
        }
    }

    fn begin_interaction(&mut self, point: Point) {
        let snapped = snap(point);
        match self.tool {
            Tool::Pencil | Tool::PixelLine => {
                if let Some(pixel) = pixel_at(point, false) {
                    self.interaction = Some(ActiveInteraction::Pixels {
                        start: pixel,
                        current: pixel,
                        path: vec![pixel],
                    });
                }
            }
            Tool::Surface | Tool::Water => {
                self.interaction = Some(ActiveInteraction::Entity {
                    start: snapped,
                    current: snapped,
                })
            }
            Tool::Light => {
                if inside(point) {
                    let id = self.block.read().map_or(1, |scene| scene.next_entity_id());
                    self.block.operate(PixelRayTracerOperation::AddEntity {
                        entity: RayEntity::Light {
                            id,
                            position: snapped,
                            color_index: self.color_index,
                            intensity: self.new_light_intensity,
                        },
                    });
                    self.selected_entity = Some(id);
                }
            }
            Tool::Select => self.select_or_drag(point),
            Tool::RayTrace => {}
        }
    }

    fn select_or_drag(&mut self, point: Point) {
        if let Some(id) = self.selected_entity {
            if let Some(entity) = self.block.read().and_then(|scene| {
                scene
                    .entities()
                    .iter()
                    .find(|item| item.id() == id)
                    .cloned()
            }) {
                let tolerance = 3.0;
                match entity {
                    RayEntity::Light { position, .. } if distance(point, position) <= tolerance => {
                        self.interaction = Some(ActiveInteraction::Light {
                            id,
                            current: position,
                        });
                        return;
                    }
                    RayEntity::Surface { start, .. } | RayEntity::Water { start, .. }
                        if distance(point, start) <= tolerance =>
                    {
                        self.interaction = Some(ActiveInteraction::Endpoint {
                            id,
                            start: true,
                            current: start,
                        });
                        return;
                    }
                    RayEntity::Surface { end, .. } | RayEntity::Water { end, .. }
                        if distance(point, end) <= tolerance =>
                    {
                        self.interaction = Some(ActiveInteraction::Endpoint {
                            id,
                            start: false,
                            current: end,
                        });
                        return;
                    }
                    _ => {}
                }
            }
        }
        self.selected_entity = self.find_entity(point).map(|entity| entity.id());
    }

    fn moved_entity(
        entities: &[RayEntity],
        id: u64,
        endpoint: Option<bool>,
        point: Point,
    ) -> Option<RayEntity> {
        let mut entity = entities.iter().find(|entity| entity.id() == id).cloned()?;
        match (&mut entity, endpoint) {
            (RayEntity::Light { position, .. }, None) => *position = point,
            (RayEntity::Surface { start, .. } | RayEntity::Water { start, .. }, Some(true)) => {
                *start = point
            }
            (RayEntity::Surface { end, .. } | RayEntity::Water { end, .. }, Some(false)) => {
                *end = point
            }
            _ => return None,
        }
        Some(entity)
    }

    fn dragged_entity(&self, entities: &[RayEntity]) -> Option<RayEntity> {
        match self.interaction.as_ref()? {
            ActiveInteraction::Endpoint { id, start, current } => {
                Self::moved_entity(entities, *id, Some(*start), *current)
            }
            ActiveInteraction::Light { id, current } => {
                Self::moved_entity(entities, *id, None, *current)
            }
            _ => None,
        }
    }

    fn finish_interaction(&mut self) {
        let Some(interaction) = self.interaction.take() else {
            return;
        };
        match interaction {
            ActiveInteraction::Pixels {
                start,
                current,
                path,
            } => {
                let points = if self.tool == Tool::Pencil {
                    path
                } else {
                    raster_line(start, current)
                };
                let pixels = points
                    .into_iter()
                    .map(|(x, y)| PixelUpdate {
                        x,
                        y,
                        color_index: self.color_index,
                    })
                    .collect();
                self.block
                    .operate(PixelRayTracerOperation::Paint { pixels });
            }
            ActiveInteraction::Entity { start, current } if start != current => {
                let id = self.block.read().map_or(1, |scene| scene.next_entity_id());
                let entity = if self.tool == Tool::Surface {
                    RayEntity::Surface {
                        id,
                        start,
                        end: current,
                        color_index: self.color_index,
                        roughness: self.new_surface_roughness,
                        metalness: self.new_surface_metalness,
                        transmission: self.new_surface_transmission,
                        refractive_index: self.new_surface_ior,
                    }
                } else {
                    RayEntity::Water {
                        id,
                        start,
                        end: current,
                    }
                };
                self.block
                    .operate(PixelRayTracerOperation::AddEntity { entity });
                self.selected_entity = Some(id);
            }
            ActiveInteraction::Endpoint { id, start, current } => {
                let entity = self.block.read().and_then(|scene| {
                    Self::moved_entity(scene.entities(), id, Some(start), current)
                });
                if let Some(entity) = entity {
                    self.block
                        .operate(PixelRayTracerOperation::UpdateEntity { entity });
                }
            }
            ActiveInteraction::Light { id, current } => {
                let entity = self
                    .block
                    .read()
                    .and_then(|scene| Self::moved_entity(scene.entities(), id, None, current));
                if let Some(entity) = entity {
                    self.block
                        .operate(PixelRayTracerOperation::UpdateEntity { entity });
                }
            }
            _ => {}
        }
    }

    fn find_entity(&self, point: Point) -> Option<RayEntity> {
        let scene = self.block.read()?;
        let mut closest: Option<(f32, RayEntity)> = None;
        for entity in scene.entities() {
            let distance = match entity {
                RayEntity::Light { position, .. } => distance(point, *position),
                RayEntity::Surface { start, end, .. } => distance_to_segment(point, *start, *end),
                RayEntity::Water { start, end, .. } => {
                    let left = start.x.min(end.x);
                    let right = start.x.max(end.x);
                    let top = start.y.min(end.y);
                    let bottom = start.y.max(end.y);
                    if (left..=right).contains(&point.x) && (top..=bottom).contains(&point.y) {
                        0.0
                    } else {
                        distance_to_segment(point, Point::new(left, top), Point::new(right, bottom))
                    }
                }
            };
            if distance <= 4.0 && closest.as_ref().is_none_or(|(old, _)| distance < *old) {
                closest = Some((distance, entity.clone()));
            }
        }
        closest.map(|(_, entity)| entity)
    }

    fn draw_entities(&self, painter: &egui::Painter, canvas: Rect) {
        let Some(scene) = self.block.read() else {
            return;
        };
        let scale = canvas.width() / f32::from(PIXEL_RAY_TRACER_SIZE);
        let draft = self.dragged_entity(scene.entities());
        for stored_entity in scene.entities() {
            let entity = draft
                .as_ref()
                .filter(|draft| draft.id() == stored_entity.id())
                .unwrap_or(stored_entity);
            let selected = self.selected_entity == Some(entity.id());
            match entity {
                RayEntity::Surface {
                    start,
                    end,
                    color_index,
                    ..
                } => {
                    let color = palette_color(*color_index);
                    painter.line_segment(
                        [
                            world_to_screen(*start, canvas),
                            world_to_screen(*end, canvas),
                        ],
                        Stroke::new((1.5 * scale).clamp(1.0, 4.0), color),
                    );
                }
                RayEntity::Water { start, end, .. } => {
                    let rect = Rect::from_two_pos(
                        world_to_screen(*start, canvas),
                        world_to_screen(*end, canvas),
                    );
                    painter.rect_filled(
                        rect,
                        0.0,
                        Color32::from_rgba_unmultiplied(41, 173, 255, 60),
                    );
                    painter.rect_stroke(
                        rect,
                        0.0,
                        Stroke::new(1.0, Color32::from_rgb(100, 220, 255)),
                        egui::StrokeKind::Inside,
                    );
                }
                RayEntity::Light {
                    position,
                    color_index,
                    ..
                } => {
                    painter.circle_filled(
                        world_to_screen(*position, canvas),
                        (2.5 * scale).clamp(3.0, 9.0),
                        palette_color(*color_index),
                    );
                    painter.circle_stroke(
                        world_to_screen(*position, canvas),
                        (2.5 * scale).clamp(3.0, 9.0),
                        Stroke::new(1.0, Color32::WHITE),
                    );
                }
            }
            if selected {
                match entity {
                    RayEntity::Light { position, .. } => {
                        painter.circle_stroke(
                            world_to_screen(*position, canvas),
                            7.0,
                            Stroke::new(2.0, Color32::YELLOW),
                        );
                    }
                    RayEntity::Surface { start, end, .. } | RayEntity::Water { start, end, .. } => {
                        for point in [start, end] {
                            painter.circle_filled(
                                world_to_screen(*point, canvas),
                                4.0,
                                Color32::WHITE,
                            );
                        }
                    }
                }
            }
        }
    }

    fn draw_interaction(&self, painter: &egui::Painter, canvas: Rect) {
        let Some(interaction) = &self.interaction else {
            return;
        };
        match interaction {
            ActiveInteraction::Pixels {
                start,
                current,
                path,
            } => {
                let points = if self.tool == Tool::Pencil {
                    path.clone()
                } else {
                    raster_line(*start, *current)
                };
                for point in points {
                    let min =
                        world_to_screen(Point::new(f32::from(point.0), f32::from(point.1)), canvas);
                    let size = canvas.width() / f32::from(PIXEL_RAY_TRACER_SIZE);
                    painter.rect_filled(
                        Rect::from_min_size(min, Vec2::splat(size)),
                        0.0,
                        palette_color(self.color_index),
                    );
                }
            }
            ActiveInteraction::Entity { start, current } if self.tool == Tool::Surface => {
                painter.line_segment(
                    [
                        world_to_screen(*start, canvas),
                        world_to_screen(*current, canvas),
                    ],
                    Stroke::new(2.0, palette_color(self.color_index)),
                );
            }
            ActiveInteraction::Entity { start, current } => {
                painter.rect_filled(
                    Rect::from_two_pos(
                        world_to_screen(*start, canvas),
                        world_to_screen(*current, canvas),
                    ),
                    0.0,
                    Color32::from_rgba_unmultiplied(41, 173, 255, 60),
                );
            }
            _ => {}
        }
    }

    fn draw_ray_trace(&mut self, painter: &egui::Painter, canvas: Rect, origin: Point) {
        let completed = self
            .ray_job
            .as_ref()
            .and_then(|receiver| match receiver.try_recv() {
                Ok(result) => Some(Ok(result)),
                Err(TryRecvError::Disconnected) => Some(Err(())),
                Err(TryRecvError::Empty) => None,
            });
        if let Some(completed) = completed {
            self.ray_job = None;
            if let Ok((origin, pixels, debug, revision, settings, include_debug, duration)) =
                completed
            {
                performance::record_duration("View-ray trace", duration);
                let image = color_image(&pixels);
                if let Some(texture) = &mut self.ray_texture {
                    texture.set(image, egui::TextureOptions::NEAREST);
                } else {
                    self.ray_texture = Some(painter.ctx().load_texture(
                        format!("pixel-rays-{}", self.block.id()),
                        image,
                        egui::TextureOptions::NEAREST,
                    ));
                }
                self.ray_overlay = Some((origin, pixels, debug, revision, settings, include_debug));
            }
        }
        let Some(scene) = self.block.read() else {
            return;
        };
        let settings = scene.view_ray_settings();
        let revision = scene.revision();
        let lighting_ready = self.texture_revision
            == Some(LightingCacheKey {
                block_revision: scene.lighting_revision(),
                preview_revision: 0,
            });
        let cache_hit = lighting_ready
            && self.ray_overlay.as_ref().is_some_and(
                |(old, _, _, old_revision, old_settings, old_debug)| {
                    distance(*old, origin) <= 0.25
                        && *old_revision == revision
                        && *old_settings == settings
                        && *old_debug == self.debug_overlay
                },
            );
        if cache_hit {
            performance::increment("View-ray cache hits");
        } else if lighting_ready && self.ray_job.is_none() {
            let source = self.rendered.clone();
            let entities = scene.entities().to_vec();
            let include_debug = self.debug_overlay;
            let repaint = painter.ctx().clone();
            let (sender, receiver) = mpsc::channel();
            thread::Builder::new()
                .name("pixel-ray-tracer-view-rays".into())
                .spawn(move || {
                    let start = Instant::now();
                    let result =
                        raytracer::trace_rays(&source, &entities, origin, settings, include_debug);
                    let _ = sender.send((
                        origin,
                        result.pixels,
                        result.debug_positions,
                        revision,
                        settings,
                        include_debug,
                        start.elapsed(),
                    ));
                    repaint.request_repaint();
                })
                .expect("failed to start pixel ray tracer view-ray job");
            self.ray_job = Some(receiver);
            performance::increment("View-ray cache misses");
        }
        drop(scene);
        if let (Some((_, _, debug, ..)), Some(texture)) = (&self.ray_overlay, &self.ray_texture) {
            painter.image(
                texture.id(),
                canvas,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            if self.debug_overlay {
                for point in debug {
                    painter.circle_filled(
                        world_to_screen(*point, canvas),
                        0.75,
                        Color32::from_rgba_unmultiplied(255, 255, 255, 90),
                    );
                }
            }
        }
    }
}

impl BlockEditor for PixelRayTracerEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }
    fn render(
        &mut self,
        context: BlockRenderContext<'_>,
        _editors: &mut super::EditorAccess<'_>,
    ) -> bool {
        if !self.ensure_texture(context.painter.ctx()) {
            return false;
        }
        self.paint_block_texture(context);
        true
    }
    fn render_aspect_ratio(&self) -> Option<f32> {
        Some(1.0)
    }
    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }
    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: true,
        }
    }
    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<Vec2> {
        Some(Vec2::splat(384.0))
    }
    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        ui.horizontal_wrapped(|ui| {
            ui.strong("Pixel Ray Tracer");
            ui.weak("128 × 128");
            ui.separator();
            if ui.button("Fit view").clicked() {
                viewport.fit();
            }
            if ui.button("Performance").clicked() {
                performance::open();
            }
            if ui.button("Reset artwork").clicked() {
                self.block.operate(PixelRayTracerOperation::Reset);
                self.selected_entity = None;
            }
        });
        None
    }
    fn direct_editor_has_left_sidebar(&self, _editors: &mut super::EditorAccess<'_>) -> bool {
        true
    }
    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.tools(ui);
        None
    }
    fn direct_editor_has_right_sidebar(&self, _editors: &mut super::EditorAccess<'_>) -> bool {
        true
    }
    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.properties(ui);
        None
    }
    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut super::EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let _performance_group = performance::GroupGuard::new(self.performance_group());
        let _frame_measurement = performance::MeasurementGuard::new("Editor frame");
        self.canvas(ui, viewport);
        None
    }
}

fn ray_settings_ui(ui: &mut egui::Ui, settings: &mut RaySettings) -> bool {
    let mut changed = ui
        .add(egui::Slider::new(&mut settings.ray_count, 1..=2048).text("Rays"))
        .changed();
    changed |= ui
        .add(
            egui::Slider::new(&mut settings.step_distance, 0.05..=128.0)
                .logarithmic(true)
                .text("Step"),
        )
        .changed();
    changed |= ui
        .add(egui::Slider::new(&mut settings.maximum_steps, 1..=2048).text("Steps"))
        .changed();
    changed
}
fn color_image(pixels: &[[u8; 4]]) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [usize::from(PIXEL_RAY_TRACER_SIZE); 2],
        &pixels.iter().flat_map(|pixel| *pixel).collect::<Vec<_>>(),
    )
}
fn palette_color(index: u8) -> Color32 {
    let rgb = PIXEL_RAY_TRACER_PALETTE[usize::from(index)];
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}
fn world_to_screen(point: Point, canvas: Rect) -> Pos2 {
    canvas.min + Vec2::new(point.x, point.y) * (canvas.width() / f32::from(PIXEL_RAY_TRACER_SIZE))
}
fn screen_to_world(point: Pos2, canvas: Rect) -> Point {
    let value = (point - canvas.min) / canvas.width() * f32::from(PIXEL_RAY_TRACER_SIZE);
    Point::new(value.x, value.y)
}
fn pixel_at(point: Point, clamp: bool) -> Option<(u16, u16)> {
    if !clamp && !inside(point) {
        return None;
    }
    Some((
        point
            .x
            .floor()
            .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1)) as u16,
        point
            .y
            .floor()
            .clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE - 1)) as u16,
    ))
}
fn snap(point: Point) -> Point {
    Point::new(
        ((point.x * 2.0).round() / 2.0).clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE)),
        ((point.y * 2.0).round() / 2.0).clamp(0.0, f32::from(PIXEL_RAY_TRACER_SIZE)),
    )
}
fn inside(point: Point) -> bool {
    point.x >= 0.0
        && point.y >= 0.0
        && point.x <= f32::from(PIXEL_RAY_TRACER_SIZE)
        && point.y <= f32::from(PIXEL_RAY_TRACER_SIZE)
}
fn distance(first: Point, second: Point) -> f32 {
    (first.x - second.x).hypot(first.y - second.y)
}
fn distance_to_segment(point: Point, start: Point, end: Point) -> f32 {
    let length = (end.x - start.x).powi(2) + (end.y - start.y).powi(2);
    if length == 0.0 {
        return distance(point, start);
    }
    let t = (((point.x - start.x) * (end.x - start.x) + (point.y - start.y) * (end.y - start.y))
        / length)
        .clamp(0.0, 1.0);
    distance(
        point,
        Point::new(
            start.x + t * (end.x - start.x),
            start.y + t * (end.y - start.y),
        ),
    )
}
fn raster_line(start: (u16, u16), end: (u16, u16)) -> Vec<(u16, u16)> {
    let (mut x, mut y) = (i32::from(start.0), i32::from(start.1));
    let (end_x, end_y) = (i32::from(end.0), i32::from(end.1));
    let dx = (end_x - x).abs();
    let sx = if x < end_x { 1 } else { -1 };
    let dy = -(end_y - y).abs();
    let sy = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;
    let mut points = Vec::new();
    loop {
        points.push((x as u16, y as u16));
        if x == end_x && y == end_y {
            break;
        }
        let twice = 2 * error;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
    points
}
