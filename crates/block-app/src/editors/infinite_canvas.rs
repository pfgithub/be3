use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};

use block::{Block, BlockParent, BlockReference, BlockReferenceList};
use block_client::{
    blocks::infinite_canvas::{
        CanvasEntity, CanvasEntityKind, CanvasLayerMove, CanvasPoint, CanvasTransform,
        InfiniteCanvas, InfiniteCanvasOperation,
    },
    BlockClient, BlockHandle, BlockRelationships, CachedBlock,
};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Stroke, Vec2};
use uuid::Uuid;

use crate::block_picker::{BlockPicker, BlockPickerMenuAction};

use super::{BlockEditor, EditorAction};

const MIN_SIZE: f32 = 4.0;
const HIT_RADIUS: f32 = 7.0;
const HANDLE_RADIUS: f32 = 5.0;
const ROTATE_OFFSET: f32 = 28.0;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Line,
    Rectangle,
    Text,
    Pen,
    Block,
}

#[derive(Clone, Copy, Debug)]
struct WorldRect {
    min: CanvasPoint,
    max: CanvasPoint,
}

impl WorldRect {
    fn from_points(a: CanvasPoint, b: CanvasPoint) -> Self {
        Self {
            min: CanvasPoint::new(a.x.min(b.x), a.y.min(b.y)),
            max: CanvasPoint::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    fn center(self) -> CanvasPoint {
        CanvasPoint::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }

    fn size(self) -> CanvasPoint {
        CanvasPoint::new(self.max.x - self.min.x, self.max.y - self.min.y)
    }

    fn union(self, other: Self) -> Self {
        Self {
            min: CanvasPoint::new(self.min.x.min(other.min.x), self.min.y.min(other.min.y)),
            max: CanvasPoint::new(self.max.x.max(other.max.x), self.max.y.max(other.max.y)),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
}

#[derive(Clone, Copy, Debug)]
struct ResizeHandle {
    x: i8,
    y: i8,
}

#[derive(Clone, Debug)]
enum Gesture {
    Create {
        tool: Tool,
        start: CanvasPoint,
        current: CanvasPoint,
    },
    Pen {
        points: Vec<CanvasPoint>,
    },
    SelectBox {
        start: CanvasPoint,
        current: CanvasPoint,
        additive: bool,
    },
    Move {
        start: CanvasPoint,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
    },
    Resize {
        handle: ResizeHandle,
        bounds: WorldRect,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
    },
    Rotate {
        bounds: WorldRect,
        start_angle: f32,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
    },
}

pub(super) struct InfiniteCanvasEditor {
    block: BlockHandle<InfiniteCanvas>,
    tool: Tool,
    camera: CanvasPoint,
    zoom: f32,
    selection: HashSet<Uuid>,
    gesture: Option<Gesture>,
    picker: BlockPicker,
    armed_block: Option<CachedBlock>,
    pending_block_center: Option<CanvasPoint>,
    context_menu_position: Option<CanvasPoint>,
    last_reference_refresh: Instant,
    focus_text: Option<Uuid>,
}

impl InfiniteCanvasEditor {
    pub(super) fn new(block: BlockHandle<InfiniteCanvas>, client: &BlockClient) -> Self {
        client.cache_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            tool: Tool::Select,
            camera: CanvasPoint::default(),
            zoom: 1.0,
            selection: HashSet::new(),
            gesture: None,
            picker: BlockPicker::default(),
            armed_block: None,
            pending_block_center: None,
            context_menu_position: None,
            last_reference_refresh: Instant::now(),
            focus_text: None,
        }
    }

    fn world_to_screen(&self, point: CanvasPoint, rect: Rect) -> Pos2 {
        rect.center()
            + Vec2::new(
                (point.x - self.camera.x) * self.zoom,
                (point.y - self.camera.y) * self.zoom,
            )
    }

    fn screen_to_world(&self, point: Pos2, rect: Rect) -> CanvasPoint {
        let relative = point - rect.center();
        CanvasPoint::new(
            self.camera.x + relative.x / self.zoom,
            self.camera.y + relative.y / self.zoom,
        )
    }

    fn selected_entities(&self, entities: &[CanvasEntity]) -> Vec<CanvasEntity> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .cloned()
            .collect()
    }

    fn selected_bounds(&self, entities: &[CanvasEntity]) -> Option<WorldRect> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    fn entity_at(&self, entities: &[CanvasEntity], point: CanvasPoint) -> Option<Uuid> {
        entities
            .iter()
            .rev()
            .find(|entity| hit_entity(entity, point, HIT_RADIUS / self.zoom))
            .map(|entity| entity.id)
    }

    fn add_entity(&mut self, entity: CanvasEntity) {
        let id = entity.id;
        if matches!(entity.kind, CanvasEntityKind::Text { .. }) {
            self.focus_text = Some(id);
        }
        self.block.operate(InfiniteCanvasOperation::Add { entity });
        self.selection.clear();
        self.selection.insert(id);
    }

    fn add_block_entity(&mut self, block_id: Uuid, center: CanvasPoint) {
        self.add_entity(CanvasEntity {
            id: Uuid::new_v4(),
            transform: CanvasTransform::new(center, CanvasPoint::new(180.0, 100.0), 0.0),
            kind: CanvasEntityKind::Block { block_id },
        });
    }

    fn show_toolbar(&mut self, ui: &mut egui::Ui, entities: &[CanvasEntity]) -> Option<Uuid> {
        let mut create_block = None;
        ui.horizontal(|ui| {
            for (tool, label) in [
                (Tool::Select, "Select"),
                (Tool::Line, "Line"),
                (Tool::Rectangle, "Rectangle"),
                (Tool::Text, "Text"),
                (Tool::Pen, "Pen"),
            ] {
                if ui.selectable_label(self.tool == tool, label).clicked() {
                    self.tool = tool;
                    self.armed_block = None;
                }
            }
            ui.menu_button("Block", |ui| {
                if let Some(action) = BlockPicker::show_menu(ui) {
                    self.tool = Tool::Block;
                    self.armed_block = None;
                    self.pending_block_center = None;
                    match action {
                        BlockPickerMenuAction::New(block_type) => {
                            create_block = Some(block_type);
                        }
                        BlockPickerMenuAction::LinkExisting => {
                            self.picker.open([self.block.id()]);
                        }
                    }
                }
            });

            if let Some(block) = &self.armed_block {
                ui.weak(format!(
                    "Place: {}",
                    if block.name.is_empty() {
                        block.id.to_string()
                    } else {
                        block.name.clone()
                    }
                ));
            }

            let selected_text = entities.iter().find(|entity| {
                self.selection.len() == 1
                    && self.selection.contains(&entity.id)
                    && matches!(entity.kind, CanvasEntityKind::Text { .. })
            });
            if let Some(entity) = selected_text {
                let CanvasEntityKind::Text { text } = &entity.kind else {
                    unreachable!();
                };
                ui.separator();
                ui.label("Text:");
                let mut edited = text.clone();
                let response = ui.add_sized([220.0, 24.0], egui::TextEdit::singleline(&mut edited));
                if self.focus_text == Some(entity.id) {
                    response.request_focus();
                    self.focus_text = None;
                }
                if response.changed() {
                    let mut updated = entity.clone();
                    updated.kind = CanvasEntityKind::Text { text: edited };
                    self.block.operate(InfiniteCanvasOperation::Update {
                        entities: vec![updated],
                    });
                }
            }
        });
        ui.separator();
        create_block
    }

    fn show_picker(&mut self, context: &egui::Context, client: &BlockClient) {
        if let Some(block) = self.picker.show(context, client) {
            if let Some(center) = self.pending_block_center.take() {
                self.add_block_entity(block.id, center);
                self.tool = Tool::Select;
            } else {
                self.armed_block = Some(block);
                self.tool = Tool::Block;
            }
        }
    }

    fn handle_zoom_and_pan(&mut self, response: &egui::Response) -> bool {
        if response.hovered() {
            if let Some(pointer) = response.ctx.pointer_hover_pos() {
                let scroll = response.ctx.input(|input| input.smooth_scroll_delta.y);
                if scroll != 0.0 {
                    let before = self.screen_to_world(pointer, response.rect);
                    self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(MIN_ZOOM, MAX_ZOOM);
                    let after = self.screen_to_world(pointer, response.rect);
                    self.camera.x += before.x - after.x;
                    self.camera.y += before.y - after.y;
                }
            }
        }

        let panning = response.ctx.input(|input| {
            input.pointer.button_down(PointerButton::Middle)
                || (input.key_down(egui::Key::Space)
                    && input.pointer.button_down(PointerButton::Primary))
        });
        if panning && response.hovered() {
            let delta = response.ctx.input(|input| input.pointer.delta());
            self.camera.x -= delta.x / self.zoom;
            self.camera.y -= delta.y / self.zoom;
            self.gesture = None;
            return true;
        }
        false
    }

    fn handle_canvas_input(
        &mut self,
        response: &egui::Response,
        entities: &[CanvasEntity],
    ) -> (Option<CanvasLayerMove>, Option<Uuid>) {
        if response
            .ctx
            .input(|input| input.key_pressed(egui::Key::Escape))
        {
            self.gesture = None;
            self.armed_block = None;
            self.picker.close();
            self.tool = Tool::Select;
        }
        if self.tool == Tool::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
            && !self.selection.is_empty()
        {
            self.block.operate(InfiniteCanvasOperation::Remove {
                ids: self.selection.drain().collect(),
            });
        }

        if self.handle_zoom_and_pan(response) {
            return (None, None);
        }

        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.ctx.pointer_hover_pos());
        let world = pointer.map(|point| self.screen_to_world(point, response.rect));

        if let Some(reference) = response.dnd_release_payload::<BlockReference>() {
            if reference.id != self.block.id() {
                if let Some(world) = world {
                    self.add_block_entity(reference.id, world);
                }
            }
        }

        if response.secondary_clicked() {
            if let Some(world) = world {
                self.context_menu_position = Some(world);
                if let Some(id) = self.entity_at(entities, world) {
                    if !self.selection.contains(&id) {
                        self.selection.clear();
                        self.selection.insert(id);
                    }
                }
            }
        }
        let mut layer_move = None;
        let mut create_block = None;
        response.context_menu(|ui| {
            ui.menu_button("Add", |ui| {
                if ui.button("Rectangle").clicked() {
                    if let Some(center) = self.context_menu_position {
                        self.add_entity(CanvasEntity {
                            id: Uuid::new_v4(),
                            transform: CanvasTransform::new(
                                center,
                                CanvasPoint::new(180.0, 100.0),
                                0.0,
                            ),
                            kind: CanvasEntityKind::Rectangle,
                        });
                    }
                    self.tool = Tool::Select;
                    ui.close();
                }
                if ui.button("Line").clicked() {
                    if let Some(center) = self.context_menu_position {
                        self.add_entity(CanvasEntity {
                            id: Uuid::new_v4(),
                            transform: CanvasTransform::new(
                                center,
                                CanvasPoint::new(180.0, MIN_SIZE),
                                0.0,
                            ),
                            kind: CanvasEntityKind::Line,
                        });
                    }
                    self.tool = Tool::Select;
                    ui.close();
                }
                ui.menu_button("Block", |ui| {
                    if let Some(action) = BlockPicker::show_menu(ui) {
                        self.tool = Tool::Block;
                        self.armed_block = None;
                        self.pending_block_center = self.context_menu_position;
                        match action {
                            BlockPickerMenuAction::New(block_type) => {
                                create_block = Some(block_type);
                            }
                            BlockPickerMenuAction::LinkExisting => {
                                self.picker.open([self.block.id()]);
                            }
                        }
                    }
                });
            });
            ui.separator();
            for (label, movement) in [
                ("Bring to front", CanvasLayerMove::BringToFront),
                ("Forwards one", CanvasLayerMove::ForwardOne),
                ("Back one", CanvasLayerMove::BackOne),
                ("Send to back", CanvasLayerMove::SendToBack),
            ] {
                if ui
                    .add_enabled(!self.selection.is_empty(), egui::Button::new(label))
                    .clicked()
                {
                    layer_move = Some(movement);
                    ui.close();
                }
            }
        });

        let Some(world) = world else {
            return (layer_move, create_block);
        };
        let primary_pressed = response
            .ctx
            .input(|input| input.pointer.button_pressed(PointerButton::Primary));
        if primary_pressed && response.hovered() {
            match self.tool {
                Tool::Select => {
                    let selected_bounds = self.selected_bounds(entities);
                    let handle = selected_bounds
                        .and_then(|bounds| resize_handle_at(self, bounds, response.rect, world));
                    let rotate = selected_bounds.is_some_and(|bounds| {
                        rotate_handle_at(self, bounds, response.rect)
                            .distance(self.world_to_screen(world, response.rect))
                            <= HANDLE_RADIUS + 3.0
                    });
                    if rotate {
                        let bounds = selected_bounds.unwrap();
                        let center = bounds.center();
                        self.gesture = Some(Gesture::Rotate {
                            bounds,
                            start_angle: (world.y - center.y).atan2(world.x - center.x),
                            current: world,
                            originals: self.selected_entities(entities),
                        });
                    } else if let (Some(bounds), Some(handle)) = (selected_bounds, handle) {
                        self.gesture = Some(Gesture::Resize {
                            handle,
                            bounds,
                            current: world,
                            originals: self.selected_entities(entities),
                        });
                    } else if let Some(id) = self.entity_at(entities, world) {
                        let additive = response.ctx.input(|input| input.modifiers.shift);
                        if additive {
                            if !self.selection.insert(id) {
                                self.selection.remove(&id);
                            }
                            self.gesture = None;
                        } else {
                            if !self.selection.contains(&id) {
                                self.selection.clear();
                                self.selection.insert(id);
                            }
                            self.gesture = Some(Gesture::Move {
                                start: world,
                                current: world,
                                originals: self.selected_entities(entities),
                            });
                        }
                    } else {
                        let additive = response.ctx.input(|input| input.modifiers.shift);
                        self.gesture = Some(Gesture::SelectBox {
                            start: world,
                            current: world,
                            additive,
                        });
                    }
                }
                Tool::Line | Tool::Rectangle => {
                    self.gesture = Some(Gesture::Create {
                        tool: self.tool,
                        start: world,
                        current: world,
                    });
                }
                Tool::Text => {
                    self.add_entity(CanvasEntity {
                        id: Uuid::new_v4(),
                        transform: CanvasTransform::new(world, CanvasPoint::new(180.0, 60.0), 0.0),
                        kind: CanvasEntityKind::Text {
                            text: "Text".into(),
                        },
                    });
                    self.tool = Tool::Select;
                }
                Tool::Pen => {
                    self.gesture = Some(Gesture::Pen {
                        points: vec![world],
                    });
                }
                Tool::Block => {
                    if let Some(block) = self.armed_block.take() {
                        self.add_block_entity(block.id, world);
                        self.tool = Tool::Select;
                    } else {
                        self.picker.open([self.block.id()]);
                    }
                }
            }
        }

        let primary_down = response
            .ctx
            .input(|input| input.pointer.button_down(PointerButton::Primary));
        if primary_down {
            match self.gesture.as_mut() {
                Some(Gesture::Create { current, .. })
                | Some(Gesture::SelectBox { current, .. })
                | Some(Gesture::Move { current, .. })
                | Some(Gesture::Resize { current, .. })
                | Some(Gesture::Rotate { current, .. }) => *current = world,
                Some(Gesture::Pen { points }) => {
                    if points
                        .last()
                        .is_none_or(|last| distance(*last, world) > 1.0 / self.zoom)
                    {
                        points.push(world);
                    }
                }
                None => {}
            }
        }

        let primary_released = response
            .ctx
            .input(|input| input.pointer.button_released(PointerButton::Primary));
        if primary_released {
            if let Some(gesture) = self.gesture.take() {
                self.finish_gesture(gesture, entities);
            }
        }
        (layer_move, create_block)
    }

    fn finish_gesture(&mut self, gesture: Gesture, entities: &[CanvasEntity]) {
        match gesture {
            Gesture::Create {
                tool,
                start,
                current,
            } => {
                let entity = match tool {
                    Tool::Line if distance(start, current) >= MIN_SIZE => {
                        let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
                        Some(CanvasEntity {
                            id: Uuid::new_v4(),
                            transform: CanvasTransform::new(
                                midpoint(start, current),
                                CanvasPoint::new(distance(start, current), 1.0),
                                delta.y.atan2(delta.x),
                            ),
                            kind: CanvasEntityKind::Line,
                        })
                    }
                    Tool::Rectangle => {
                        let bounds = WorldRect::from_points(start, current);
                        Some(CanvasEntity {
                            id: Uuid::new_v4(),
                            transform: CanvasTransform::new(
                                bounds.center(),
                                CanvasPoint::new(
                                    bounds.size().x.max(MIN_SIZE),
                                    bounds.size().y.max(MIN_SIZE),
                                ),
                                0.0,
                            ),
                            kind: CanvasEntityKind::Rectangle,
                        })
                    }
                    _ => None,
                };
                if let Some(entity) = entity {
                    self.add_entity(entity);
                }
            }
            Gesture::Pen { points } => {
                if points.len() >= 2 {
                    self.add_entity(pen_entity(points));
                }
            }
            Gesture::SelectBox {
                start,
                current,
                additive,
            } => {
                if !additive {
                    self.selection.clear();
                }
                let selection = WorldRect::from_points(start, current);
                self.selection.extend(
                    entities
                        .iter()
                        .filter(|entity| entity_bounds(entity).intersects(selection))
                        .map(|entity| entity.id),
                );
            }
            Gesture::Move { .. } | Gesture::Resize { .. } | Gesture::Rotate { .. } => {
                let updates = preview_entities(&gesture);
                if !updates.is_empty() {
                    self.block
                        .operate(InfiniteCanvasOperation::Update { entities: updates });
                }
            }
        }
    }

    fn paint(&self, painter: &egui::Painter, rect: Rect, entities: &[CanvasEntity]) {
        let preview = self
            .gesture
            .as_ref()
            .map(preview_entities)
            .unwrap_or_default();
        let preview: HashMap<_, _> = preview
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
        for stored in entities {
            let entity = preview.get(&stored.id).unwrap_or(stored);
            paint_entity(
                self,
                painter,
                rect,
                entity,
                self.selection.contains(&entity.id),
            );
        }

        if let Some(Gesture::Create {
            tool,
            start,
            current,
        }) = &self.gesture
        {
            let preview_stroke =
                Stroke::new(2.0, painter.ctx().global_style().visuals.text_color());
            match tool {
                Tool::Line => {
                    painter.line_segment(
                        [
                            self.world_to_screen(*start, rect),
                            self.world_to_screen(*current, rect),
                        ],
                        preview_stroke,
                    );
                }
                Tool::Rectangle => {
                    let selection = WorldRect::from_points(*start, *current);
                    painter.rect_stroke(
                        screen_rect(self, selection, rect),
                        0.0,
                        preview_stroke,
                        egui::StrokeKind::Inside,
                    );
                }
                _ => {}
            }
        }
        if let Some(Gesture::Pen { points }) = &self.gesture {
            painter.add(egui::Shape::line(
                points
                    .iter()
                    .map(|point| self.world_to_screen(*point, rect))
                    .collect(),
                Stroke::new(2.0, painter.ctx().global_style().visuals.text_color()),
            ));
        }
        if let Some(Gesture::SelectBox { start, current, .. }) = &self.gesture {
            let selection = screen_rect(self, WorldRect::from_points(*start, *current), rect);
            painter.rect_filled(
                selection,
                0.0,
                Color32::from_rgba_unmultiplied(66, 153, 225, 28),
            );
            painter.rect_stroke(
                selection,
                0.0,
                Stroke::new(1.0, Color32::LIGHT_BLUE),
                egui::StrokeKind::Inside,
            );
        }

        if let Some(bounds) = self.selected_bounds_with_preview(entities, &preview) {
            paint_selection(self, painter, rect, bounds);
        }
    }

    fn selected_bounds_with_preview(
        &self,
        entities: &[CanvasEntity],
        preview: &HashMap<Uuid, CanvasEntity>,
    ) -> Option<WorldRect> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .map(|entity| preview.get(&entity.id).unwrap_or(entity))
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    fn selected_block_action(
        &mut self,
        context: &egui::Context,
        rect: Rect,
        entities: &[CanvasEntity],
        client: &BlockClient,
    ) -> Option<EditorAction> {
        if self.selection.len() != 1 {
            return None;
        }
        let entity = entities
            .iter()
            .find(|entity| self.selection.contains(&entity.id))?;
        let CanvasEntityKind::Block { block_id } = entity.kind else {
            return None;
        };
        let bounds = entity_bounds(entity);
        let position =
            self.world_to_screen(CanvasPoint::new(bounds.center().x, bounds.max.y), rect);
        let cached = client.cached_block(block_id);
        if cached.is_none() && self.last_reference_refresh.elapsed() >= Duration::from_secs(1) {
            client.cache_references(BlockReferenceList::References(self.block.id()));
            self.last_reference_refresh = Instant::now();
        }
        let mut action = None;
        egui::Area::new(egui::Id::new(("open-canvas-block", entity.id)))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(position + Vec2::new(0.0, 6.0))
            .show(context, |ui| {
                if ui
                    .add_enabled(cached.is_some(), egui::Button::new("Open block"))
                    .on_disabled_hover_text("Waiting for cached block metadata")
                    .clicked()
                {
                    let cached = cached.as_ref().unwrap();
                    action = Some(EditorAction::OpenBlock {
                        id: cached.id,
                        block_type: cached.block_type,
                    });
                }
            });
        action
    }
}

impl BlockEditor for InfiniteCanvasEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        InfiniteCanvas::TYPE_ID
    }

    fn name(&self) -> String {
        self.block.name()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.block.read().map(|_| self.block.relationships())
    }

    fn set_parent(&self, parent: BlockParent) {
        self.block.set_parent(parent);
    }

    fn note_backref(&self, id: Uuid) {
        self.block.note_backref(id);
    }

    fn block_created(&mut self, id: Uuid, block_type: Uuid, name: String) {
        if let Some(center) = self.pending_block_center.take() {
            self.add_block_entity(id, center);
            self.tool = Tool::Select;
        } else {
            self.armed_block = Some(CachedBlock {
                id,
                block_type,
                name,
            });
            self.tool = Tool::Block;
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, client: &BlockClient) -> Option<EditorAction> {
        let Some(canvas) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let entities = canvas.entities().to_vec();
        drop(canvas);

        let mut create_block = self.show_toolbar(ui, &entities);
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let (layer_move, context_create_block) = self.handle_canvas_input(&response, &entities);
        create_block = create_block.or(context_create_block);
        if let Some(movement) = layer_move {
            self.block.operate(InfiniteCanvasOperation::Reorder {
                ids: self.selection.iter().copied().collect(),
                movement,
            });
        }
        let painted_entities = self
            .block
            .read()
            .map(|canvas| canvas.entities().to_vec())
            .unwrap_or(entities);
        self.paint(&painter, response.rect, &painted_entities);
        self.show_picker(ui.ctx(), client);
        let action = self.selected_block_action(ui.ctx(), response.rect, &painted_entities, client);
        ui.ctx().request_repaint();
        action.or_else(|| {
            create_block.map(|block_type| EditorAction::CreateBlock {
                block_type,
                parent: Some(self.block.id()),
            })
        })
    }
}

fn midpoint(a: CanvasPoint, b: CanvasPoint) -> CanvasPoint {
    CanvasPoint::new((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

fn distance(a: CanvasPoint, b: CanvasPoint) -> f32 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn rotate(point: CanvasPoint, angle: f32) -> CanvasPoint {
    let (sin, cos) = angle.sin_cos();
    CanvasPoint::new(point.x * cos - point.y * sin, point.x * sin + point.y * cos)
}

fn local_to_world(transform: CanvasTransform, local: CanvasPoint) -> CanvasPoint {
    let scaled = CanvasPoint::new(local.x * transform.size.x, local.y * transform.size.y);
    let rotated = rotate(scaled, transform.rotation);
    CanvasPoint::new(
        transform.center.x + rotated.x,
        transform.center.y + rotated.y,
    )
}

fn world_to_local(transform: CanvasTransform, world: CanvasPoint) -> CanvasPoint {
    let relative = CanvasPoint::new(world.x - transform.center.x, world.y - transform.center.y);
    let rotated = rotate(relative, -transform.rotation);
    CanvasPoint::new(
        rotated.x / transform.size.x.max(0.001),
        rotated.y / transform.size.y.max(0.001),
    )
}

fn entity_corners(entity: &CanvasEntity) -> [CanvasPoint; 4] {
    [
        local_to_world(entity.transform, CanvasPoint::new(-0.5, -0.5)),
        local_to_world(entity.transform, CanvasPoint::new(0.5, -0.5)),
        local_to_world(entity.transform, CanvasPoint::new(0.5, 0.5)),
        local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.5)),
    ]
}

fn entity_bounds(entity: &CanvasEntity) -> WorldRect {
    match &entity.kind {
        CanvasEntityKind::Line => {
            let start = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
            let end = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
            WorldRect::from_points(start, end)
        }
        CanvasEntityKind::Pen { points } if !points.is_empty() => points
            .iter()
            .map(|point| local_to_world(entity.transform, *point))
            .map(|point| WorldRect::from_points(point, point))
            .reduce(WorldRect::union)
            .unwrap(),
        _ => {
            let corners = entity_corners(entity);
            corners
                .into_iter()
                .map(|point| WorldRect::from_points(point, point))
                .reduce(WorldRect::union)
                .unwrap()
        }
    }
}

fn hit_entity(entity: &CanvasEntity, point: CanvasPoint, radius: f32) -> bool {
    match &entity.kind {
        CanvasEntityKind::Line => {
            let a = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
            let b = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
            point_segment_distance(point, a, b) <= radius
        }
        CanvasEntityKind::Pen { points } => points.windows(2).any(|segment| {
            point_segment_distance(
                point,
                local_to_world(entity.transform, segment[0]),
                local_to_world(entity.transform, segment[1]),
            ) <= radius
        }),
        _ => {
            let local = world_to_local(entity.transform, point);
            local.x.abs() <= 0.5 + radius / entity.transform.size.x.max(MIN_SIZE)
                && local.y.abs() <= 0.5 + radius / entity.transform.size.y.max(MIN_SIZE)
        }
    }
}

fn point_segment_distance(point: CanvasPoint, a: CanvasPoint, b: CanvasPoint) -> f32 {
    let ab = CanvasPoint::new(b.x - a.x, b.y - a.y);
    let length_squared = ab.x * ab.x + ab.y * ab.y;
    if length_squared <= f32::EPSILON {
        return distance(point, a);
    }
    let projection =
        (((point.x - a.x) * ab.x + (point.y - a.y) * ab.y) / length_squared).clamp(0.0, 1.0);
    distance(
        point,
        CanvasPoint::new(a.x + ab.x * projection, a.y + ab.y * projection),
    )
}

fn screen_rect(editor: &InfiniteCanvasEditor, bounds: WorldRect, rect: Rect) -> Rect {
    Rect::from_two_pos(
        editor.world_to_screen(bounds.min, rect),
        editor.world_to_screen(bounds.max, rect),
    )
}

fn resize_handle_at(
    editor: &InfiniteCanvasEditor,
    bounds: WorldRect,
    rect: Rect,
    world: CanvasPoint,
) -> Option<ResizeHandle> {
    let pointer = editor.world_to_screen(world, rect);
    resize_handles(bounds)
        .into_iter()
        .find_map(|(handle, point)| {
            (editor.world_to_screen(point, rect).distance(pointer) <= HANDLE_RADIUS + 3.0)
                .then_some(handle)
        })
}

fn resize_handles(bounds: WorldRect) -> [(ResizeHandle, CanvasPoint); 8] {
    let center = bounds.center();
    [
        (
            ResizeHandle { x: -1, y: -1 },
            CanvasPoint::new(bounds.min.x, bounds.min.y),
        ),
        (
            ResizeHandle { x: 0, y: -1 },
            CanvasPoint::new(center.x, bounds.min.y),
        ),
        (
            ResizeHandle { x: 1, y: -1 },
            CanvasPoint::new(bounds.max.x, bounds.min.y),
        ),
        (
            ResizeHandle { x: 1, y: 0 },
            CanvasPoint::new(bounds.max.x, center.y),
        ),
        (
            ResizeHandle { x: 1, y: 1 },
            CanvasPoint::new(bounds.max.x, bounds.max.y),
        ),
        (
            ResizeHandle { x: 0, y: 1 },
            CanvasPoint::new(center.x, bounds.max.y),
        ),
        (
            ResizeHandle { x: -1, y: 1 },
            CanvasPoint::new(bounds.min.x, bounds.max.y),
        ),
        (
            ResizeHandle { x: -1, y: 0 },
            CanvasPoint::new(bounds.min.x, center.y),
        ),
    ]
}

fn rotate_handle_at(editor: &InfiniteCanvasEditor, bounds: WorldRect, rect: Rect) -> Pos2 {
    let top = editor.world_to_screen(CanvasPoint::new(bounds.center().x, bounds.min.y), rect);
    top - Vec2::new(0.0, ROTATE_OFFSET)
}

fn preview_entities(gesture: &Gesture) -> Vec<CanvasEntity> {
    match gesture {
        Gesture::Move {
            start,
            current,
            originals,
        } => {
            let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
            originals
                .iter()
                .cloned()
                .map(|mut entity| {
                    entity.transform.center.x += delta.x;
                    entity.transform.center.y += delta.y;
                    entity
                })
                .collect()
        }
        Gesture::Resize {
            handle,
            bounds,
            current,
            originals,
        } => resize_entities(*handle, *bounds, *current, originals),
        Gesture::Rotate {
            bounds,
            start_angle,
            current,
            originals,
        } => {
            let center = bounds.center();
            let current_angle = (current.y - center.y).atan2(current.x - center.x);
            let delta = current_angle - start_angle;
            originals
                .iter()
                .cloned()
                .map(|mut entity| {
                    let relative = CanvasPoint::new(
                        entity.transform.center.x - center.x,
                        entity.transform.center.y - center.y,
                    );
                    let rotated = rotate(relative, delta);
                    entity.transform.center =
                        CanvasPoint::new(center.x + rotated.x, center.y + rotated.y);
                    entity.transform.rotation += delta;
                    entity
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

fn resize_entities(
    handle: ResizeHandle,
    bounds: WorldRect,
    current: CanvasPoint,
    originals: &[CanvasEntity],
) -> Vec<CanvasEntity> {
    let original_size = bounds.size();
    let mut resized = bounds;
    if handle.x < 0 {
        resized.min.x = current.x.min(resized.max.x - MIN_SIZE);
    } else if handle.x > 0 {
        resized.max.x = current.x.max(resized.min.x + MIN_SIZE);
    }
    if handle.y < 0 {
        resized.min.y = current.y.min(resized.max.y - MIN_SIZE);
    } else if handle.y > 0 {
        resized.max.y = current.y.max(resized.min.y + MIN_SIZE);
    }
    let resized_size = resized.size();
    let scale_x = if handle.x == 0 {
        1.0
    } else {
        resized_size.x / original_size.x.max(MIN_SIZE)
    };
    let scale_y = if handle.y == 0 {
        1.0
    } else {
        resized_size.y / original_size.y.max(MIN_SIZE)
    };

    originals
        .iter()
        .cloned()
        .map(|mut entity| {
            if matches!(entity.kind, CanvasEntityKind::Line) {
                let start = local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0));
                let end = local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0));
                let resize_point = |point: CanvasPoint| {
                    CanvasPoint::new(
                        if handle.x == 0 {
                            point.x
                        } else {
                            resized.min.x
                                + (point.x - bounds.min.x) / original_size.x.max(MIN_SIZE)
                                    * resized_size.x
                        },
                        if handle.y == 0 {
                            point.y
                        } else {
                            resized.min.y
                                + (point.y - bounds.min.y) / original_size.y.max(MIN_SIZE)
                                    * resized_size.y
                        },
                    )
                };
                let start = resize_point(start);
                let end = resize_point(end);
                let delta = CanvasPoint::new(end.x - start.x, end.y - start.y);
                entity.transform.center = midpoint(start, end);
                entity.transform.size.x = distance(start, end).max(MIN_SIZE);
                entity.transform.rotation = delta.y.atan2(delta.x);
                return entity;
            }
            let unit_x = (entity.transform.center.x - bounds.min.x) / original_size.x.max(MIN_SIZE);
            let unit_y = (entity.transform.center.y - bounds.min.y) / original_size.y.max(MIN_SIZE);
            entity.transform.center = CanvasPoint::new(
                if handle.x == 0 {
                    entity.transform.center.x
                } else {
                    resized.min.x + unit_x * resized_size.x
                },
                if handle.y == 0 {
                    entity.transform.center.y
                } else {
                    resized.min.y + unit_y * resized_size.y
                },
            );
            entity.transform.size.x = (entity.transform.size.x * scale_x).max(MIN_SIZE);
            entity.transform.size.y = (entity.transform.size.y * scale_y).max(MIN_SIZE);
            entity
        })
        .collect()
}

fn pen_entity(points: Vec<CanvasPoint>) -> CanvasEntity {
    let bounds = points
        .iter()
        .copied()
        .map(|point| WorldRect::from_points(point, point))
        .reduce(WorldRect::union)
        .unwrap();
    let center = bounds.center();
    let size = CanvasPoint::new(bounds.size().x.max(MIN_SIZE), bounds.size().y.max(MIN_SIZE));
    let points = points
        .into_iter()
        .map(|point| CanvasPoint::new((point.x - center.x) / size.x, (point.y - center.y) / size.y))
        .collect();
    CanvasEntity {
        id: Uuid::new_v4(),
        transform: CanvasTransform::new(center, size, 0.0),
        kind: CanvasEntityKind::Pen { points },
    }
}

fn paint_entity(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    entity: &CanvasEntity,
    selected: bool,
) {
    let color = if selected {
        Color32::LIGHT_BLUE
    } else {
        painter.ctx().global_style().visuals.text_color()
    };
    let stroke = Stroke::new(2.0, color);
    match &entity.kind {
        CanvasEntityKind::Line => {
            painter.line_segment(
                [
                    editor.world_to_screen(
                        local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0)),
                        rect,
                    ),
                    editor.world_to_screen(
                        local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0)),
                        rect,
                    ),
                ],
                stroke,
            );
        }
        CanvasEntityKind::Rectangle => {
            let mut points: Vec<_> = entity_corners(entity)
                .into_iter()
                .map(|point| editor.world_to_screen(point, rect))
                .collect();
            points.push(points[0]);
            painter.add(egui::Shape::line(points, stroke));
        }
        CanvasEntityKind::Text { text } => {
            let center = editor.world_to_screen(entity.transform.center, rect);
            let font = egui::FontId::proportional((18.0 * editor.zoom).clamp(8.0, 48.0));
            let galley = painter.layout_no_wrap(text.clone(), font, color);
            let position = center - galley.size() * 0.5;
            painter.add(
                egui::epaint::TextShape::new(position, galley, color)
                    .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
        }
        CanvasEntityKind::Pen { points } => {
            painter.add(egui::Shape::line(
                points
                    .iter()
                    .map(|point| {
                        editor.world_to_screen(local_to_world(entity.transform, *point), rect)
                    })
                    .collect(),
                stroke,
            ));
        }
        CanvasEntityKind::Block { .. } => {
            let corners: Vec<_> = entity_corners(entity)
                .into_iter()
                .map(|point| editor.world_to_screen(point, rect))
                .collect();
            painter.add(egui::Shape::convex_polygon(
                corners,
                Color32::from_gray(35),
                stroke,
            ));
            let center = editor.world_to_screen(entity.transform.center, rect);
            let galley = painter.layout_no_wrap(
                "TODO".into(),
                egui::FontId::proportional((18.0 * editor.zoom).clamp(8.0, 42.0)),
                color,
            );
            let position = center - galley.size() * 0.5;
            painter.add(
                egui::epaint::TextShape::new(position, galley, color)
                    .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
        }
    }
}

fn paint_selection(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    bounds: WorldRect,
) {
    let screen = screen_rect(editor, bounds, rect);
    painter.rect_stroke(
        screen,
        0.0,
        Stroke::new(1.0, Color32::LIGHT_BLUE),
        egui::StrokeKind::Outside,
    );
    for (_, point) in resize_handles(bounds) {
        painter.circle_filled(
            editor.world_to_screen(point, rect),
            HANDLE_RADIUS,
            Color32::LIGHT_BLUE,
        );
    }
    let rotate = rotate_handle_at(editor, bounds, rect);
    painter.line_segment(
        [Pos2::new(screen.center().x, screen.top()), rotate],
        Stroke::new(1.0, Color32::LIGHT_BLUE),
    );
    painter.circle_filled(rotate, HANDLE_RADIUS, Color32::LIGHT_BLUE);
}
