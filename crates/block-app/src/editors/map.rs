mod geo;
// Tiles are downloaded, decoded, and rasterised on a worker thread that the
// browser build does not start yet, so this pipeline is unreachable there.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod mvt;
mod points;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod raster;
mod sidebar;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
mod tiles;

use std::cell::Cell;
use std::collections::HashMap;

use block::{BlockParent, BlockReferenceList};
use block_client::{
    blocks::{
        image::Image as ImageBlock,
        map::{Map, MapCoordinate, MapOperation, MapPoint, MapRegion, MAX_LATITUDE},
        workspace_index::BlockEntry,
    },
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2};
use egui_material_icons::{
    icons::{
        ICON_ADD, ICON_ARROW_BACK, ICON_CROP_FREE, ICON_DELETE, ICON_MAP, ICON_MY_LOCATION,
        ICON_REFRESH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
    },
    MaterialIcon,
};
use uuid::Uuid;

use crate::block_picker::BlockPicker;

use self::geo::MapView;
use self::raster::{TileLabel, TILE_PIXELS};
use self::tiles::{TileId, TileWorker, SOURCE_MAX_ZOOM};

use super::{
    clipboard::{ClipboardImagePaste, ClipboardImagePasteResult},
    image::create_image_block,
    BlockEditor, BlockRenderContext, CreatableEditor, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind, SidebarDragPayload,
};

/// Size of the whole world (the intrinsic content) at 100% viewport zoom.
const WORLD_POINTS: f32 = 1024.0;
/// Screen size of one tile at an integer map zoom level.
const TILE_POINTS: f32 = 256.0;
/// Viewport zoom that reaches the deepest source tiles: zoom 14 tiles are
/// 256 points each, so full detail spans 2^14 tiles * 256 points against the
/// 1024 point world.
const MAX_VIEWPORT_ZOOM: f32 = 4096.0;
/// Previews zoom no further than the tab viewport can, which keeps tile
/// placement inside the precision an f32 screen rect can carry.
const MAX_PREVIEW_WORLD: f64 = (WORLD_POINTS * MAX_VIEWPORT_ZOOM) as f64;
const ZOOM_STEP: f32 = 1.25;
const BACKGROUND: Color32 = Color32::from_rgb(242, 239, 233);
const REGION_COLOR: Color32 = Color32::from_rgb(245, 180, 60);
const ATTRIBUTION: &str = "© OpenStreetMap contributors";

impl EditorKind for MapEditor {
    type Block = Map;

    const DISPLAY_NAME: &'static str = "Map";
    const ICON: MaterialIcon = ICON_MAP;
    const CAN_ADD_CHILD: bool = true;
    const CAN_DELETE_CHILD: bool = true;
    const CAN_REPLACE_CHILD: bool = true;

    fn open(client: &BlockClient, block: BlockHandle<Map>) -> Self {
        Self::new(block, client)
    }
}

impl CreatableEditor for MapEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(Map::new()), client)
    }
}

enum TileState {
    Loading,
    Ready {
        texture: TextureHandle,
        labels: Vec<TileLabel>,
    },
    Failed,
}

impl TileState {
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
    last_error: Option<String>,
    dependencies: ReferenceList,
    picker: BlockPicker,
    selected: Option<Uuid>,
    /// The marker being dragged, with the offset from the pointer to its tip.
    dragged: Option<(Uuid, Vec2)>,
    /// Where the next block added through the picker should land.
    pending_position: Option<MapCoordinate>,
    pending_file_drop: Option<MapCoordinate>,
    clipboard_image_paste: ClipboardImagePaste,
    import_error: Option<String>,
    fit_region_requested: bool,
    center_requested: Option<MapCoordinate>,
    grouped_edit_active: bool,
    /// The area the direct editor last showed, used when capturing a region.
    visible_region: MapRegion,
    /// The centre of that area, readable from the shared `add_child` path.
    view_center: Cell<MapCoordinate>,
}

impl MapEditor {
    pub(super) fn new(block: BlockHandle<Map>, client: &BlockClient) -> Self {
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            worker: None,
            tiles: HashMap::new(),
            last_error: None,
            dependencies,
            picker: BlockPicker::default(),
            selected: None,
            dragged: None,
            pending_position: None,
            pending_file_drop: None,
            clipboard_image_paste: ClipboardImagePaste::default(),
            import_error: None,
            fit_region_requested: true,
            center_requested: None,
            grouped_edit_active: false,
            visible_region: MapRegion::WORLD,
            view_center: Cell::new(MapCoordinate::default()),
        }
    }

    fn points(&self) -> Vec<MapPoint> {
        self.block
            .read()
            .map(|map| map.points().to_vec())
            .unwrap_or_default()
    }

    fn preview_region(&self) -> Option<MapRegion> {
        self.block.read().and_then(|map| map.preview_region())
    }

    fn displayed_region(&self) -> MapRegion {
        self.block
            .read()
            .map_or(MapRegion::WORLD, |map| map.displayed_region())
    }

    /// Labels of the blocks the map points at, for marker labels and lists.
    fn dependency_labels(&self, editors: &EditorAccess<'_>) -> HashMap<Uuid, super::BlockLabel> {
        self.dependencies
            .read()
            .into_iter()
            .map(|reference| {
                (
                    reference.id,
                    super::BlockLabel::for_reference(editors.registry(), &reference),
                )
            })
            .collect()
    }

    fn ensure_point_editors(&self, points: &[MapPoint], editors: &mut EditorAccess<'_>) {
        for reference in self.dependencies.read() {
            if points.iter().any(|point| point.block_id == reference.id) {
                editors.ensure(reference.id, reference.block_type);
            }
        }
    }

    fn record(&mut self, operation: MapOperation) {
        self.grouped_edit_active = false;
        self.block.finish_history_group();
        self.block.operate(operation);
    }

    fn record_grouped(&mut self, operation: MapOperation) {
        self.grouped_edit_active = true;
        self.block.operate_grouped([operation]);
    }

    fn add_point(&mut self, block_id: Uuid, position: MapCoordinate) {
        let point = MapPoint::new(block_id, position);
        self.record(MapOperation::AddPoint { point });
        self.selected = Some(point.id);
    }

    fn remove_point(&mut self, id: Uuid) {
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.record(MapOperation::RemovePoints { ids: vec![id] });
    }

    fn poll(&mut self, context: &egui::Context) {
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
                },
                Err(message) => {
                    self.last_error = Some(message);
                    TileState::Failed
                }
            };
            self.tiles.insert(result.id, state);
        }
    }

    fn ensure_tile(&mut self, id: TileId) {
        let state = self.tiles.entry(id).or_insert(TileState::Loading);
        // Keep re-requesting loading tiles: the worker serves the most
        // recent requests first, so the tiles still in view stay ahead of
        // any backlog from earlier views.
        if matches!(state, TileState::Loading) {
            if let Some(worker) = &self.worker {
                worker.request(id);
            }
        }
    }

    /// Draws the world map into `world_rect`, clipped to `clip`.
    fn draw_map(&mut self, painter: &egui::Painter, world_rect: Rect, clip: Rect, opacity: f32) {
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

        let tint = Color32::WHITE.gamma_multiply(opacity);
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
                self.draw_tile(painter, id, rect, tint, opacity);
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
                    draw_label(painter, position, label, opacity);
                }
            }
        }
    }

    /// Draws one tile, falling back to a magnified ancestor while it loads.
    fn draw_tile(
        &self,
        painter: &egui::Painter,
        id: TileId,
        rect: Rect,
        tint: Color32,
        opacity: f32,
    ) {
        let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        if let Some(texture) = self.tiles.get(&id).and_then(TileState::texture) {
            painter.image(texture.id(), rect, full_uv, tint);
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
            painter.image(texture.id(), rect, uv, tint);
            return;
        }
        painter.rect_filled(rect, 0.0, BACKGROUND.gamma_multiply(opacity));
    }

    /// Places a block that arrived by drag, paste, or picker on the map.
    fn drop_position(&self, response: &egui::Response, view: MapView) -> MapCoordinate {
        response
            .ctx
            .pointer_latest_pos()
            .filter(|position| response.rect.contains(*position))
            .map_or_else(
                || self.view_center.get(),
                |position| view.coordinate(position),
            )
    }

    fn import_dropped_images(
        &mut self,
        response: &egui::Response,
        view: MapView,
        editors: &mut EditorAccess<'_>,
    ) {
        let (hovering_file, dropped) = response.ctx.input(|input| {
            (
                !input.raw.hovered_files.is_empty(),
                input.raw.dropped_files.clone(),
            )
        });
        if hovering_file {
            if let Some(position) = response
                .ctx
                .pointer_hover_pos()
                .filter(|position| response.rect.contains(*position))
            {
                self.pending_file_drop = Some(view.coordinate(position));
            }
        }
        if dropped.is_empty() {
            if !hovering_file {
                self.pending_file_drop = None;
            }
            return;
        }
        self.import_error = None;
        let base = self
            .pending_file_drop
            .take()
            .unwrap_or_else(|| self.drop_position(response, view));
        // Spread several files out so their markers do not sit on top of
        // each other, using a step scaled to how far the map is zoomed in.
        let step = (self.visible_region.east - self.visible_region.west) * 0.03;
        for (index, file) in dropped.into_iter().enumerate() {
            let position = MapCoordinate::new(
                base.longitude + step * index as f64,
                base.latitude - step * index as f64,
            );
            let source_name = file
                .path
                .as_ref()
                .and_then(|path| path.file_name())
                .and_then(|name| name.to_str())
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .or_else(|| (!file.name.is_empty()).then_some(file.name))
                .unwrap_or_else(|| "Image".into());
            let bytes = match file.bytes {
                Some(bytes) => bytes.to_vec(),
                None => match file.path.as_ref().map(std::fs::read) {
                    Some(Ok(bytes)) => bytes,
                    Some(Err(error)) => {
                        self.import_error = Some(format!("Could not read {source_name}: {error}"));
                        continue;
                    }
                    None => {
                        self.import_error =
                            Some(format!("No image data was available for {source_name}"));
                        continue;
                    }
                },
            };
            match ImageBlock::from_compressed(source_name.clone(), bytes) {
                Ok(image) => self.add_imported_image(editors, image, position),
                Err(error) => {
                    self.import_error = Some(format!("Could not import {source_name}: {error}"));
                }
            }
        }
    }

    fn import_clipboard_image(
        &mut self,
        response: &egui::Response,
        view: MapView,
        editors: &mut EditorAccess<'_>,
    ) {
        let enabled = !response.ctx.egui_wants_keyboard_input();
        let Some(result) = self.clipboard_image_paste.poll(&response.ctx, enabled) else {
            return;
        };
        let ClipboardImagePasteResult::Image(image) = result else {
            if let ClipboardImagePasteResult::Error(error) = result {
                self.import_error = Some(error);
            }
            return;
        };
        self.import_error = None;
        let position = self.drop_position(response, view);
        self.add_imported_image(editors, image, position);
    }

    fn add_imported_image(
        &mut self,
        editors: &mut EditorAccess<'_>,
        image: ImageBlock,
        position: MapCoordinate,
    ) {
        let id = create_image_block(editors, image, self.block.id());
        self.add_point(id, position);
    }

    fn handle_picker(&mut self, context: &egui::Context, editors: &mut EditorAccess<'_>) {
        let Some(result) = self
            .picker
            .handle(context, editors, BlockParent::Uuid(self.block.id()))
        else {
            return;
        };
        editors.set_parent(result.id, BlockParent::Uuid(self.block.id()));
        let position = self
            .pending_position
            .take()
            .unwrap_or_else(|| self.view_center.get());
        self.add_point(result.id, position);
    }

    fn handle_input(
        &mut self,
        response: &egui::Response,
        view: MapView,
        points: &[MapPoint],
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) {
        if let Some(dragged) = response.dnd_hover_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                response.ctx.set_cursor_icon(egui::CursorIcon::Alias);
            }
        }
        if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                let position = self.drop_position(response, view);
                self.add_point(dragged.reference.id, position);
                editors.set_parent(dragged.reference.id, BlockParent::Uuid(self.block.id()));
            }
        }
        self.import_dropped_images(response, view, editors);
        self.import_clipboard_image(response, view, editors);

        if response.drag_started() {
            self.dragged = response
                .interact_pointer_pos()
                .and_then(|pointer| points::point_at(points, view, pointer).map(|id| (id, pointer)))
                .and_then(|(id, pointer)| {
                    let point = points.iter().find(|point| point.id == id)?;
                    Some((id, view.position(point.position) - pointer))
                });
            if let Some((id, _)) = self.dragged {
                self.selected = Some(id);
            }
        }
        if let Some((id, offset)) = self.dragged {
            if let Some(pointer) = response.interact_pointer_pos() {
                if let Some(mut point) = points.iter().copied().find(|point| point.id == id) {
                    let position = view.coordinate(pointer + offset);
                    if position != point.position {
                        point.position = position;
                        self.record_grouped(MapOperation::UpdatePoints {
                            points: vec![point],
                        });
                    }
                }
            }
            if response.drag_stopped() {
                self.dragged = None;
                self.grouped_edit_active = false;
                self.block.finish_history_group();
            }
        } else if response.dragged() {
            viewport.pan(response.drag_delta());
        }
        if response.clicked() {
            self.selected = response
                .interact_pointer_pos()
                .and_then(|pointer| points::point_at(points, view, pointer));
        }
        if response.secondary_clicked() {
            if let Some(pointer) = response.interact_pointer_pos() {
                self.pending_position = Some(view.coordinate(pointer));
                if let Some(id) = points::point_at(points, view, pointer) {
                    self.selected = Some(id);
                }
            }
        }
        let selected = self.selected;
        let mut remove = None;
        let block_id = self.block.id();
        let picker = &mut self.picker;
        response.context_menu(|ui| {
            if ui
                .button(format!("{} Add here", ICON_ADD.codepoint))
                .clicked()
            {
                picker.open([block_id]);
                ui.close();
            }
            if ui
                .add_enabled(
                    selected.is_some(),
                    egui::Button::new("Remove point of interest"),
                )
                .clicked()
            {
                remove = selected;
                ui.close();
            }
        });
        if let Some(id) = remove {
            self.remove_point(id);
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
    }

    /// Draws the preview region into `rect`, cropping whatever falls outside
    /// it. This is what hosts that cannot pan and zoom - canvases, thumbnails,
    /// and slide playback - show.
    fn draw_preview_region_view(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        editors: &mut EditorAccess<'_>,
        opacity: f32,
    ) {
        self.poll(&painter.ctx().clone());
        painter.rect_filled(rect, 0.0, BACKGROUND.gamma_multiply(opacity));
        let view = MapView::covering(self.displayed_region(), rect, MAX_PREVIEW_WORLD);
        let points = self.points();
        let labels = self.dependency_labels(editors);
        self.ensure_point_editors(&points, editors);
        self.draw_map(painter, view.world_rect(), rect, opacity);
        points::draw_points(
            painter,
            view,
            rect,
            &points,
            |id| {
                labels
                    .get(&id)
                    .map(|label| (label.name.clone(), label.automatic))
            },
            None,
            opacity,
        );
    }

    /// Zooms and pans the host viewport so the preview region fills the view.
    fn fit_preview_region(
        &self,
        viewport: &mut DirectEditorViewport,
        view: MapView,
        clip: Rect,
        region: MapRegion,
    ) {
        let region_rect = view.region_rect(region);
        let available = (clip.size() - Vec2::splat(24.0)).max(Vec2::splat(1.0));
        let factor = (available.x / region_rect.width().max(0.01))
            .min(available.y / region_rect.height().max(0.01));
        viewport.change_zoom(factor, Some(region_rect.center()));
        viewport.pan(clip.center() - region_rect.center());
    }
}

fn draw_label(painter: &egui::Painter, position: Pos2, label: &TileLabel, opacity: f32) {
    let font = FontId::proportional(label.font_size);
    let halo = Color32::from_rgba_unmultiplied(255, 255, 255, 160).gamma_multiply(opacity);
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
        Color32::from_rgb(label.color[0], label.color[1], label.color[2]).gamma_multiply(opacity),
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

fn draw_region_outline(painter: &egui::Painter, rect: Rect) {
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.5_f32, REGION_COLOR),
        egui::StrokeKind::Inside,
    );
    painter.text(
        rect.left_top() + Vec2::new(5.0, 4.0),
        egui::Align2::LEFT_TOP,
        "Preview region",
        FontId::proportional(12.0),
        REGION_COLOR,
    );
}

impl BlockEditor for MapEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let map = self.block.read()?;
        let already_placed = map.points().iter().any(|point| point.block_id == entry.id);
        drop(map);
        if !already_placed {
            self.block.finish_history_group();
            self.block.operate(MapOperation::AddPoint {
                point: MapPoint::new(entry.id, self.view_center.get()),
            });
        }
        Some(true)
    }

    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        let map = self.block.read()?;
        let ids = map
            .points()
            .iter()
            .filter(|point| point.block_id == entry.id)
            .map(|point| point.id)
            .collect::<Vec<_>>();
        drop(map);
        if !ids.is_empty() {
            self.block.finish_history_group();
            self.block.operate(MapOperation::RemovePoints { ids });
        }
        Some(true)
    }

    fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        let map = self.block.read()?;
        let points = map
            .points()
            .iter()
            .filter(|point| point.block_id == old)
            .map(|point| MapPoint {
                block_id: new.id,
                ..*point
            })
            .collect::<Vec<_>>();
        drop(map);
        if !points.is_empty() {
            self.block.finish_history_group();
            self.block.operate(MapOperation::UpdatePoints { points });
        }
        Some(true)
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let rect = Rect::from_points(&context.corners);
        if !rect.is_positive() {
            return false;
        }
        let painter = context
            .painter
            .with_clip_rect(rect.intersect(context.painter.clip_rect()));
        self.draw_preview_region_view(&painter, rect, editors, context.opacity.clamp(0.0, 1.0));
        true
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        Some(geo::region_aspect_ratio(self.displayed_region()))
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

    fn direct_editor_max_zoom(&self) -> f32 {
        MAX_VIEWPORT_ZOOM
    }

    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let aspect = geo::region_aspect_ratio(self.displayed_region());
        Some(if aspect >= 1.0 {
            Vec2::new(WORLD_POINTS, WORLD_POINTS / aspect)
        } else {
            Vec2::new(WORLD_POINTS * aspect, WORLD_POINTS)
        })
    }

    /// Inside a canvas the map has no viewport of its own, so it shows the
    /// preview region instead of a pannable world.
    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (response, painter) = ui.allocate_painter(
            ui.available_size().max(Vec2::splat(1.0)),
            Sense::click_and_drag(),
        );
        let rect = response.rect;
        let painter = painter.with_clip_rect(rect.intersect(ui.clip_rect()));
        self.draw_preview_region_view(&painter, rect, editors, 1.0);
        draw_attribution(&painter, rect);
        None
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        ui.horizontal(|ui| {
            if ui
                .button(ICON_ADD)
                .on_hover_text("Add a point of interest at the centre of the view")
                .clicked()
            {
                self.pending_position = None;
                self.picker.open([self.block.id()]);
            }
            ui.separator();
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
                .add_enabled(
                    self.preview_region().is_some(),
                    egui::Button::new(ICON_CROP_FREE),
                )
                .on_hover_text("Zoom to the preview region")
                .on_disabled_hover_text("This map has no preview region")
                .clicked()
            {
                self.fit_region_requested = true;
            }
            if ui
                .button(ICON_REFRESH)
                .on_hover_text("Reload tiles")
                .clicked()
            {
                // Drop the worker too: it remembers which tiles it already
                // served and would refuse to fetch them again.
                self.worker = None;
                self.tiles.clear();
                self.last_error = None;
            }
            if let Some(error) = &self.last_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        None
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.show_sidebar(ui, editors)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let (response, painter) = ui.allocate_painter(
            ui.available_size().max(Vec2::splat(1.0)),
            Sense::click_and_drag(),
        );
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
        let view = MapView::from_world_rect(world_rect);
        self.visible_region = view.region(clip);
        self.view_center.set(view.coordinate(clip.center()));

        self.poll(ui.ctx());
        self.draw_map(&painter, world_rect, clip, 1.0);
        draw_attribution(&painter, clip);

        let points = self.points();
        let labels = self.dependency_labels(editors);
        self.ensure_point_editors(&points, editors);
        if let Some(region) = self.preview_region() {
            draw_region_outline(&painter, view.region_rect(region));
        }
        points::draw_points(
            &painter,
            view,
            clip,
            &points,
            |id| {
                labels
                    .get(&id)
                    .map(|label| (label.name.clone(), label.automatic))
            },
            self.selected,
            1.0,
        );

        // Wait for the block to load before spending the pending fit, so a map
        // opened before its data arrives still zooms to its region.
        if self.fit_region_requested && self.block.read().is_some() {
            self.fit_region_requested = false;
            if let Some(region) = self.preview_region() {
                self.fit_preview_region(viewport, view, clip, region);
            }
        }
        if let Some(coordinate) = self.center_requested.take() {
            viewport.pan(clip.center() - view.position(coordinate));
        }

        self.handle_input(&response, view, &points, editors, viewport);
        self.handle_picker(ui.ctx(), editors);
        if self.grouped_edit_active && ui.ctx().input(|input| input.pointer.any_released()) {
            self.grouped_edit_active = false;
            self.block.finish_history_group();
        }
        None
    }
}
