use std::cell::Cell;
use std::collections::HashMap;
use std::sync::Arc;

use block::{BlockParent, BlockReferenceList};
use block_client::block_ref::BlockRef;
use block_client::blocks::image::Image as ImageBlock;
use block_client::blocks::map::{Map, MapColor, MapCoordinate, MapOperation, MapPoint, MapRegion};
use block_client::references::{ReferenceClassificationQueue, ReferenceResolutionCache};
use block_client::{BlockClient, BlockHandle, ReferenceList};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::egui::{
    self, Color32, FontId, Pos2, Rect, Sense, Stroke, TextureHandle, Vec2,
};
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ADD, ICON_CROP_FREE, ICON_REFRESH, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_plugin::{
    BlockFilter, BlockPicker, ChildMode, EditorHost, ImagePaster, PastedImage,
};
use uuid::Uuid;

use crate::geo::MapView;
use crate::points;
use crate::raster::{TileLabel, TILE_PIXELS};
use crate::tiles::{TileId, TileWorker, SOURCE_MAX_ZOOM};

const WORLD_POINTS: f32 = 1024.0;
const TILE_POINTS: f32 = 256.0;
const MAX_VIEWPORT_ZOOM: f32 = 4096.0;
const MAX_PREVIEW_WORLD: f64 = (WORLD_POINTS * MAX_VIEWPORT_ZOOM) as f64;
const ZOOM_STEP: f32 = 1.25;
const BACKGROUND: Color32 = Color32::from_rgb(242, 239, 233);
pub(crate) const REGION_COLOR: Color32 = Color32::from_rgb(245, 180, 60);
const ATTRIBUTION: &str = "© OpenStreetMap contributors";

pub(crate) enum TileState {
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

struct Editing {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<Map>,
    dependencies: ReferenceList,
}

pub struct MapApp {
    editing: Option<Editing>,
    creation: Option<Arc<BlockClient>>,
    worker: Option<TileWorker>,
    tiles: HashMap<TileId, TileState>,
    pub(crate) last_error: Option<String>,
    picker: BlockPicker,
    pub(crate) selected: Option<Uuid>,
    dragged: Option<(Uuid, Vec2)>,
    pending_position: Option<MapCoordinate>,
    pending_file_drop: Option<MapCoordinate>,
    paster: ImagePaster,
    pub(crate) import_error: Option<String>,
    pub(crate) fit_region_requested: bool,
    pub(crate) center_requested: Option<MapCoordinate>,
    grouped_edit_active: bool,
    pub(crate) visible_region: MapRegion,
    view_center: Cell<MapCoordinate>,
    reference_cache: ReferenceResolutionCache,
    pending_points: ReferenceClassificationQueue<(Uuid, MapCoordinate)>,
}

impl Default for MapApp {
    fn default() -> Self {
        Self {
            editing: None,
            creation: None,
            worker: None,
            tiles: HashMap::new(),
            last_error: None,
            picker: BlockPicker::default(),
            selected: None,
            dragged: None,
            pending_position: None,
            pending_file_drop: None,
            paster: ImagePaster::default(),
            import_error: None,
            fit_region_requested: true,
            center_requested: None,
            grouped_edit_active: false,
            visible_region: MapRegion::WORLD,
            view_center: Cell::new(MapRegion::WORLD.center()),
            reference_cache: ReferenceResolutionCache::default(),
            pending_points: ReferenceClassificationQueue::default(),
        }
    }
}

impl MapApp {
    pub(crate) fn host_handle(&self) -> Option<EditorHost> {
        self.host().cloned()
    }

    pub(crate) fn host_block_types(&self) -> Option<std::rc::Rc<BlockCatalog>> {
        self.host().map(EditorHost::block_types)
    }

    pub(crate) fn block(&self) -> Option<&BlockHandle<Map>> {
        self.editing.as_ref().map(|editing| &editing.block)
    }

    pub(crate) fn block_id(&self) -> Uuid {
        self.block().map_or_else(Uuid::nil, BlockHandle::id)
    }

    fn host(&self) -> Option<&EditorHost> {
        self.editing.as_ref().map(|editing| &editing.host)
    }

    fn client(&self) -> Option<Arc<BlockClient>> {
        self.editing
            .as_ref()
            .map(|editing| Arc::clone(&editing.client))
    }
    pub(crate) fn points(&self) -> Vec<MapPoint> {
        self.block()
            .and_then(BlockHandle::read)
            .map(|map| map.points().to_vec())
            .unwrap_or_default()
    }

    pub(crate) fn preview_region(&self) -> Option<MapRegion> {
        self.block()
            .and_then(BlockHandle::read)
            .and_then(|map| map.preview_region())
    }

    pub(crate) fn displayed_region(&self) -> MapRegion {
        self.block()
            .and_then(BlockHandle::read)
            .map_or(MapRegion::WORLD, |map| map.displayed_region())
    }

    pub(crate) fn dependencies(&self) -> Vec<block::BlockReference> {
        self.editing
            .as_ref()
            .map(|editing| editing.dependencies.read())
            .unwrap_or_default()
    }

    pub(crate) fn dependency_labels(&self, types: &BlockCatalog) -> HashMap<Uuid, BlockLabel> {
        self.dependencies()
            .into_iter()
            .map(|reference| (reference.id, BlockLabel::for_reference(types, &reference)))
            .collect()
    }

    pub(crate) fn resolve_points(
        &mut self,
        points: &[MapPoint],
    ) -> HashMap<BlockRef, Option<Uuid>> {
        self.reference_cache.poll();
        let Some(client) = self.client() else {
            return HashMap::new();
        };
        let referencing_id = self.block_id();
        points
            .iter()
            .map(|point| {
                (
                    point.block_id,
                    self.reference_cache
                        .resolve(&client, referencing_id, point.block_id),
                )
            })
            .collect()
    }

    pub(crate) fn record(&mut self, operation: MapOperation) {
        self.grouped_edit_active = false;
        if let Some(block) = self.block() {
            block.finish_history_group();
        }
        if let Some(block) = self.block() {
            block.operate(operation);
        }
    }

    pub(crate) fn record_grouped(&mut self, operation: MapOperation) {
        self.grouped_edit_active = true;
        if let Some(block) = self.block() {
            block.operate_grouped([operation]);
        }
    }

    fn poll_pending_points(&mut self) {
        for (reference, (point_id, position)) in self.pending_points.poll() {
            let point = MapPoint {
                id: point_id,
                block_id: reference,
                position,
                color: MapColor::Default,
            };
            self.record(MapOperation::AddPoint { point });
        }
    }

    pub(crate) fn add_point(&mut self, block_id: Uuid, position: MapCoordinate) {
        let Some(client) = self.client() else {
            return;
        };
        let point_id = Uuid::new_v4();
        let referencing_id = self.block_id();
        self.pending_points
            .push(&client, referencing_id, block_id, (point_id, position));
        self.selected = Some(point_id);
    }

    pub(crate) fn remove_point(&mut self, id: Uuid) {
        if self.selected == Some(id) {
            self.selected = None;
        }
        self.record(MapOperation::RemovePoints { ids: vec![id] });
    }

    fn poll(&mut self, context: &egui::Context) {
        self.poll_pending_points();
        let Some(host) = self.host().cloned() else {
            return;
        };
        let worker = self
            .worker
            .get_or_insert_with(|| TileWorker::spawn(host.waker()));
        for result in worker.poll(&host) {
            let state = match result.result {
                Ok(raster) => TileState::Ready {
                    texture: context.load_texture(
                        format!(
                            "map-tile-{}-{}-{}-{}",
                            self.block_id(),
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

        if matches!(state, TileState::Loading) {
            if let Some(worker) = &mut self.worker {
                worker.request(id);
            }
        }
    }

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

    fn import_dropped_images(&mut self, view: MapView) {
        let Some(drop) = self.host().and_then(EditorHost::files) else {
            self.pending_file_drop = None;
            return;
        };
        if !drop.dropped {
            self.pending_file_drop = Some(view.coordinate(drop.position));
            return;
        }
        self.import_error = None;
        let base = self
            .pending_file_drop
            .take()
            .unwrap_or_else(|| view.coordinate(drop.position));
        let step = (self.visible_region.east - self.visible_region.west) * 0.03;
        for (index, file) in drop.files.into_iter().enumerate() {
            let position = MapCoordinate::new(
                base.longitude + step * index as f64,
                base.latitude - step * index as f64,
            );
            self.add_imported_image(ImageBlock::new(file.name, file.data), position);
        }
    }

    fn import_clipboard_image(&mut self, ui: &egui::Ui) {
        let Some(host) = self.host().cloned() else {
            return;
        };
        let enabled = !ui.ctx().egui_wants_keyboard_input();
        let Some(pasted) = self.paster.poll(ui, &host, enabled) else {
            return;
        };
        match pasted {
            PastedImage::Image { name, data } => {
                self.import_error = None;
                let position = self.view_center.get();
                self.add_imported_image(ImageBlock::new(name, data), position);
            }
            PastedImage::Failed(error) => self.import_error = Some(error),
            PastedImage::Empty => {}
        }
    }

    fn add_imported_image(&mut self, image: ImageBlock, position: MapCoordinate) {
        let Some(client) = self.client() else {
            return;
        };
        let created = client.create_block(image);
        created.set_parent(BlockParent::Uuid(self.block_id()));
        self.add_point(created.id(), position);
    }

    fn handle_picker(&mut self) {
        let Some(host) = self.host().cloned() else {
            return;
        };
        let Some(Ok((id, _))) = self.picker.poll(&host) else {
            return;
        };
        if let Some(client) = self.client() {
            client.set_block_parent(id, BlockParent::Uuid(self.block_id()));
        }
        let position = self
            .pending_position
            .take()
            .unwrap_or_else(|| self.view_center.get());
        self.add_point(id, position);
    }

    fn handle_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        view: MapView,
        points: &[MapPoint],
    ) {
        let host = self.host().cloned();
        if let (Some(host), Some(dragged)) = (&host, host.as_ref().and_then(EditorHost::drag)) {
            if dragged.block_id != self.block_id() {
                host.accept_drag(true);
                if dragged.dropped {
                    let position = view.coordinate(dragged.position);
                    if let Some(client) = self.client() {
                        client
                            .set_block_parent(dragged.block_id, BlockParent::Uuid(self.block_id()));
                    }
                    self.add_point(dragged.block_id, position);
                }
            }
        }
        self.import_dropped_images(view);
        self.import_clipboard_image(ui);

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
                if let Some(block) = self.block() {
                    block.finish_history_group();
                }
            }
        } else if response.dragged() {
            if let Some(host) = self.host() {
                host.pan_view(response.drag_delta());
            }
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
        let mut add_here = false;
        response.context_menu(|ui| {
            if ui
                .button(format!("{} Add here", ICON_ADD.codepoint))
                .clicked()
            {
                add_here = true;
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
        if add_here {
            if let Some(host) = self.host().cloned() {
                self.picker.open(&host, BlockFilter::default());
            }
        }
        if let Some(id) = remove {
            self.remove_point(id);
        }
    }

    fn draw_preview_region_view(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        types: &BlockCatalog,
        opacity: f32,
    ) {
        self.poll(&painter.ctx().clone());
        painter.rect_filled(rect, 0.0, BACKGROUND.gamma_multiply(opacity));
        let view = MapView::covering(self.displayed_region(), rect, MAX_PREVIEW_WORLD);
        let points = self.points();
        let labels = self.dependency_labels(types);
        let resolved = self.resolve_points(&points);
        self.draw_map(painter, view.world_rect(), rect, opacity);
        points::draw_points(
            painter,
            view,
            rect,
            &points,
            |block_id| {
                resolved
                    .get(&block_id)
                    .copied()
                    .flatten()
                    .and_then(|id| labels.get(&id))
                    .map(|label| (label.name.clone(), label.automatic))
            },
            None,
            opacity,
        );
    }

    fn fit_preview_region(&self, view: MapView, clip: Rect, region: MapRegion) {
        let Some(host) = self.host() else {
            return;
        };
        let region_rect = view.region_rect(region);
        let available = (clip.size() - Vec2::splat(24.0)).max(Vec2::splat(1.0));
        let factor = (available.x / region_rect.width().max(0.01))
            .min(available.y / region_rect.height().max(0.01));
        host.zoom_view(factor, Some(region_rect.center()));
        host.pan_view(clip.center() - region_rect.center());
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

impl block_editor_plugin::App for MapApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.fit_region_requested = true;
        self.visible_region = MapRegion::WORLD;
        self.editing = Some(Editing {
            host,
            block: client.get_block(block_id),
            dependencies: client.watch_references(BlockReferenceList::References(block_id)),
            client,
        });
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Map::new()).id())
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        Some(crate::geo::region_aspect_ratio(self.displayed_region()))
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let aspect = crate::geo::region_aspect_ratio(self.displayed_region());
        Some(match aspect >= 1.0 {
            true => Vec2::new(WORLD_POINTS, WORLD_POINTS / aspect),
            false => Vec2::new(WORLD_POINTS * aspect, WORLD_POINTS),
        })
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let Some(types) = self.host().map(EditorHost::block_types) else {
            return;
        };
        let rect = ui.max_rect();
        if !rect.is_positive() {
            return;
        }
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        self.draw_preview_region_view(&painter, rect, types.as_ref(), 1.0);
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(host) = self.host().cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            if ui
                .button(ICON_ADD)
                .on_hover_text("Add a point of interest at the centre of the view")
                .test_id("map.add-point")
                .clicked()
            {
                self.pending_position = None;
                self.picker.open(&host, BlockFilter::default());
            }
            ui.separator();
            if ui.button(ICON_ZOOM_OUT).on_hover_text("Zoom out").clicked() {
                host.zoom_view(1.0 / ZOOM_STEP, None);
            }
            if ui.button(ICON_ZOOM_IN).on_hover_text("Zoom in").clicked() {
                host.zoom_view(ZOOM_STEP, None);
            }
            if ui.button("Whole world").clicked() {
                host.fit_view();
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
                self.worker = None;
                self.tiles.clear();
                self.last_error = None;
            }
            if let Some(error) = &self.last_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.show_sidebar(ui);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(types)) = (
            self.host().cloned(),
            self.host().map(EditorHost::block_types),
        ) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let (response, painter) = ui.allocate_painter(
            ui.available_size().max(Vec2::splat(1.0)),
            Sense::click_and_drag(),
        );
        let clip = response.rect.intersect(ui.clip_rect());
        let painter = painter.with_clip_rect(clip);
        painter.rect_filled(clip, 0.0, ui.visuals().extreme_bg_color);

        let content_rect = host.view().unwrap_or(response.rect);
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
        let labels = self.dependency_labels(types.as_ref());
        let resolved = self.resolve_points(&points);
        if let Some(region) = self.preview_region() {
            draw_region_outline(&painter, view.region_rect(region));
        }
        points::draw_points(
            &painter,
            view,
            clip,
            &points,
            |block_id| {
                resolved
                    .get(&block_id)
                    .copied()
                    .flatten()
                    .and_then(|id| labels.get(&id))
                    .map(|label| (label.name.clone(), label.automatic))
            },
            self.selected,
            1.0,
        );

        if self.fit_region_requested && self.block().and_then(BlockHandle::read).is_some() {
            self.fit_region_requested = false;
            if let Some(region) = self.preview_region() {
                self.fit_preview_region(view, clip, region);
            }
        }
        if let Some(coordinate) = self.center_requested.take() {
            host.pan_view(clip.center() - view.position(coordinate));
        }

        self.handle_input(ui, &response, view, &points);
        self.handle_picker();
        if self.grouped_edit_active && ui.ctx().input(|input| input.pointer.any_released()) {
            self.grouped_edit_active = false;
            if let Some(block) = self.block() {
                block.finish_history_group();
            }
        }
    }
}

pub(crate) fn place_preview(
    ui: &mut egui::Ui,
    host: &EditorHost,
    rect: Rect,
    block_id: Uuid,
    block_type: Uuid,
) -> block_editor_plugin::ChildHandle {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("map-preview", block_id))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let handle = host.child_sized(&mut child, rect.size(), block_id, block_type);
    handle.set_mode(ChildMode::Preview);
    handle
}
