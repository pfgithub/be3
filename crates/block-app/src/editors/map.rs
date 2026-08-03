mod mvt;
mod raster;
mod tiles;

use std::collections::HashMap;

use block::Block;
use block_client::{blocks::map::Map, BlockHandle};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, TextureHandle, Vec2};
use egui_material_icons::icons::{ICON_MAP, ICON_REFRESH, ICON_ZOOM_IN, ICON_ZOOM_OUT};

use self::raster::{TileLabel, TILE_PIXELS};
use self::tiles::{TileId, TileWorker, SOURCE_MAX_ZOOM};

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorRegistration,
};

/// Size of the whole world (the intrinsic content) at 100% viewport zoom.
const WORLD_POINTS: f32 = 1024.0;
/// Screen size of one tile at an integer map zoom level.
const TILE_POINTS: f32 = 256.0;
const MAX_CACHED_TILES: usize = 128;
/// Tiles at or below this zoom are never evicted; they back previews and
/// serve as fallbacks while deeper tiles load.
const EVICTION_MIN_ZOOM: u8 = 2;
const ZOOM_STEP: f32 = 1.25;
const BACKGROUND: Color32 = Color32::from_rgb(242, 239, 233);
const ATTRIBUTION: &str = "© OpenStreetMap contributors";

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: Map::TYPE_ID,
        display_name: "Map",
        icon: ICON_MAP,
        create: Some(|client| Box::new(MapEditor::new(client.create_block(Map::new())))),
        open: |client, id| Box::new(MapEditor::new(client.get_block::<Map>(id))),
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

enum TileState {
    Loading {
        last_used: u64,
    },
    Ready {
        texture: TextureHandle,
        labels: Vec<TileLabel>,
        last_used: u64,
    },
    Failed {
        last_used: u64,
    },
}

impl TileState {
    fn touch(&mut self, frame: u64) {
        match self {
            TileState::Loading { last_used }
            | TileState::Ready { last_used, .. }
            | TileState::Failed { last_used } => *last_used = frame,
        }
    }

    fn last_used(&self) -> u64 {
        match self {
            TileState::Loading { last_used }
            | TileState::Ready { last_used, .. }
            | TileState::Failed { last_used } => *last_used,
        }
    }

    fn texture(&self) -> Option<&TextureHandle> {
        match self {
            TileState::Ready { texture, .. } => Some(texture),
            _ => None,
        }
    }
}

pub(super) struct MapEditor {
    block: BlockHandle<Map>,
    worker: Option<TileWorker>,
    tiles: HashMap<TileId, TileState>,
    frame: u64,
    last_error: Option<String>,
}

impl MapEditor {
    pub(super) fn new(block: BlockHandle<Map>) -> Self {
        Self {
            block,
            worker: None,
            tiles: HashMap::new(),
            frame: 0,
            last_error: None,
        }
    }

    fn poll(&mut self, context: &egui::Context) {
        self.frame += 1;
        let worker = self
            .worker
            .get_or_insert_with(|| TileWorker::spawn(context.clone()));
        while let Some(result) = worker.try_result() {
            let state = match result.result {
                Ok(raster) => TileState::Ready {
                    texture: context.load_texture(
                        format!(
                            "map-tile-{}-{}-{}-{}",
                            self.block.id(),
                            result.id.zoom,
                            result.id.x,
                            result.id.y
                        ),
                        egui::ColorImage::from_rgba_unmultiplied(
                            [TILE_PIXELS, TILE_PIXELS],
                            &raster.pixels,
                        ),
                        egui::TextureOptions::LINEAR,
                    ),
                    labels: raster.labels,
                    last_used: self.frame,
                },
                Err(message) => {
                    self.last_error = Some(message);
                    TileState::Failed {
                        last_used: self.frame,
                    }
                }
            };
            self.tiles.insert(result.id, state);
        }
    }

    fn ensure_tile(&mut self, id: TileId) {
        let frame = self.frame;
        self.tiles
            .entry(id)
            .and_modify(|state| state.touch(frame))
            .or_insert_with(|| {
                if let Some(worker) = &self.worker {
                    worker.request(id);
                }
                TileState::Loading { last_used: frame }
            });
    }

    fn evict_stale_tiles(&mut self) {
        if self.tiles.len() <= MAX_CACHED_TILES {
            return;
        }
        let mut candidates: Vec<(TileId, u64)> = self
            .tiles
            .iter()
            .filter(|(id, state)| id.zoom > EVICTION_MIN_ZOOM && state.last_used() < self.frame)
            .map(|(id, state)| (*id, state.last_used()))
            .collect();
        candidates.sort_by_key(|(_, last_used)| *last_used);
        let excess = self.tiles.len().saturating_sub(MAX_CACHED_TILES);
        for (id, _) in candidates.into_iter().take(excess) {
            self.tiles.remove(&id);
        }
    }

    /// Draws the world map into `world_rect`, clipped to `clip`.
    fn draw_map(&mut self, painter: &egui::Painter, world_rect: Rect, clip: Rect) {
        let visible = clip.intersect(world_rect);
        if !visible.is_positive() {
            return;
        }
        let zoom = (world_rect.width().max(1.0) / TILE_POINTS).log2();
        let tile_zoom = zoom.floor().clamp(0.0, f32::from(SOURCE_MAX_ZOOM)) as u8;
        let tile_count = 1u32 << tile_zoom;
        let tile_points = world_rect.width() / tile_count as f32;
        let range = |from: f32, to: f32| {
            let from = ((from / tile_points).floor().max(0.0) as u32).min(tile_count);
            let to = ((to / tile_points).ceil().max(0.0) as u32).min(tile_count);
            from..to
        };
        let columns = range(
            visible.left() - world_rect.left(),
            visible.right() - world_rect.left(),
        );
        let rows = range(
            visible.top() - world_rect.top(),
            visible.bottom() - world_rect.top(),
        );

        for y in rows.clone() {
            for x in columns.clone() {
                self.ensure_tile(TileId {
                    zoom: tile_zoom,
                    x,
                    y,
                });
            }
        }

        for y in rows.clone() {
            for x in columns.clone() {
                let id = TileId {
                    zoom: tile_zoom,
                    x,
                    y,
                };
                let rect = Rect::from_min_max(
                    world_rect.min + Vec2::new(x as f32 * tile_points, y as f32 * tile_points),
                    world_rect.min
                        + Vec2::new((x + 1) as f32 * tile_points, (y + 1) as f32 * tile_points),
                );
                self.draw_tile(painter, id, rect);
            }
        }

        for y in rows {
            for x in columns.clone() {
                let id = TileId {
                    zoom: tile_zoom,
                    x,
                    y,
                };
                let Some(TileState::Ready { labels, .. }) = self.tiles.get(&id) else {
                    continue;
                };
                let origin =
                    world_rect.min + Vec2::new(x as f32 * tile_points, y as f32 * tile_points);
                for label in labels {
                    let position = origin
                        + Vec2::new(label.position[0], label.position[1]) * tile_points
                            / TILE_PIXELS as f32;
                    if !clip.expand(40.0).contains(position) {
                        continue;
                    }
                    draw_label(painter, position, label);
                }
            }
        }
    }

    /// Draws one tile, falling back to a magnified ancestor while it loads.
    fn draw_tile(&self, painter: &egui::Painter, id: TileId, rect: Rect) {
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        if let Some(texture) = self.tiles.get(&id).and_then(TileState::texture) {
            painter.image(texture.id(), rect, full_uv, Color32::WHITE);
            return;
        }
        let mut ancestor = id;
        while let Some(parent) = ancestor.parent() {
            ancestor = parent;
            let Some(texture) = self.tiles.get(&ancestor).and_then(TileState::texture) else {
                continue;
            };
            let magnification = 1u32 << (id.zoom - ancestor.zoom);
            let uv_size = 1.0 / magnification as f32;
            let uv = Rect::from_min_size(
                Pos2::new(
                    (id.x - ancestor.x * magnification) as f32 * uv_size,
                    (id.y - ancestor.y * magnification) as f32 * uv_size,
                ),
                Vec2::splat(uv_size),
            );
            painter.image(texture.id(), rect, uv, Color32::WHITE);
            return;
        }
        painter.rect_filled(rect, 0.0, BACKGROUND);
    }
}

fn draw_label(painter: &egui::Painter, position: Pos2, label: &TileLabel) {
    let font = FontId::proportional(label.font_size);
    let halo = Color32::from_rgba_unmultiplied(255, 255, 255, 160);
    for offset in [
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
    ] {
        painter.text(
            position + offset,
            egui::Align2::CENTER_CENTER,
            &label.text,
            font.clone(),
            halo,
        );
    }
    painter.text(
        position,
        egui::Align2::CENTER_CENTER,
        &label.text,
        font,
        Color32::from_rgb(label.color[0], label.color[1], label.color[2]),
    );
}

fn draw_attribution(painter: &egui::Painter, clip: Rect) {
    let galley = painter.layout_no_wrap(
        ATTRIBUTION.into(),
        FontId::proportional(10.0),
        Color32::from_gray(80),
    );
    let position = clip.right_bottom() - galley.size().to_pos2().to_vec2() - Vec2::new(6.0, 4.0);
    painter.rect_filled(
        Rect::from_min_size(position, galley.size()).expand(3.0),
        2.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 210),
    );
    painter.galley(position, galley, Color32::from_gray(80));
}

impl BlockEditor for MapEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(&mut self, context: BlockRenderContext<'_>, _editors: &mut EditorAccess<'_>) -> bool {
        self.poll(&context.painter.ctx().clone());
        let world = TileId {
            zoom: 0,
            x: 0,
            y: 0,
        };
        self.ensure_tile(world);
        let opacity = (context.opacity.clamp(0.0, 1.0) * 255.0).round() as u8;
        if let Some(texture) = self.tiles.get(&world).and_then(TileState::texture) {
            let tint = Color32::from_white_alpha(opacity);
            let mut mesh = egui::Mesh::with_texture(texture.id());
            let uvs = [
                Pos2::new(0.0, 0.0),
                Pos2::new(1.0, 0.0),
                Pos2::new(1.0, 1.0),
                Pos2::new(0.0, 1.0),
            ];
            for (corner, uv) in context.corners.iter().zip(uvs) {
                mesh.vertices.push(egui::epaint::Vertex {
                    pos: *corner,
                    uv,
                    color: tint,
                });
            }
            mesh.indices.extend([0, 1, 2, 0, 2, 3]);
            context.painter.add(mesh);
        } else {
            context.painter.add(egui::Shape::convex_polygon(
                context.corners.to_vec(),
                Color32::from_rgba_unmultiplied(
                    BACKGROUND.r(),
                    BACKGROUND.g(),
                    BACKGROUND.b(),
                    opacity,
                ),
                egui::Stroke::NONE,
            ));
        }
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

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        Some(Vec2::splat(WORLD_POINTS))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        ui.horizontal(|ui| {
            if ui.button(ICON_ZOOM_OUT).on_hover_text("Zoom out").clicked() {
                viewport.change_zoom(1.0 / ZOOM_STEP, None);
            }
            if ui.button(ICON_ZOOM_IN).on_hover_text("Zoom in").clicked() {
                viewport.change_zoom(ZOOM_STEP, None);
            }
            if ui.button("Whole world").clicked() {
                viewport.fit();
            }
            if ui
                .button(ICON_REFRESH)
                .on_hover_text("Reload tiles")
                .clicked()
            {
                self.tiles.clear();
                self.last_error = None;
            }
            if let Some(error) = &self.last_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (response, painter) =
            ui.allocate_painter(ui.available_size().max(Vec2::splat(1.0)), Sense::drag());
        let clip = response.rect.intersect(ui.clip_rect());
        let painter = painter.with_clip_rect(clip);
        painter.rect_filled(clip, 0.0, ui.visuals().extreme_bg_color);

        // The host lets the content rect grow past the intrinsic square, so
        // place the square world centered inside it.
        let content_rect = viewport.content_rect().unwrap_or(response.rect);
        let world_rect = Rect::from_center_size(
            content_rect.center(),
            Vec2::splat(content_rect.width().min(content_rect.height())),
        );
        self.poll(ui.ctx());
        self.draw_map(&painter, world_rect, clip);
        self.evict_stale_tiles();
        draw_attribution(&painter, clip);

        if response.dragged() {
            viewport.pan(response.drag_delta());
        }
        if response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let (scroll, zoom_delta, command) = response.ctx.input(|input| {
                    (
                        input.smooth_scroll_delta,
                        input.zoom_delta(),
                        input.modifiers.command,
                    )
                });
                if (zoom_delta - 1.0).abs() > f32::EPSILON {
                    viewport.change_zoom(zoom_delta, Some(pointer));
                } else if command && scroll.y != 0.0 {
                    viewport.change_zoom((scroll.y * 0.002).exp(), Some(pointer));
                } else if scroll != Vec2::ZERO {
                    viewport.pan(scroll);
                }
            }
        }
        None
    }
}
