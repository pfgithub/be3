use std::collections::{HashMap, HashSet};

use block::{Block, BlockReferenceList};
use block_client::{
    blocks::{
        image::Image as ImageBlock,
        infinite_canvas::{
            CanvasColor, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasLayerMove,
            CanvasPoint, CanvasTextAlign, CanvasTextStyle, CanvasTextWeight, CanvasTransform,
            InfiniteCanvas, InfiniteCanvasOperation,
        },
    },
    BlockClient, BlockHandle, CachedBlock, ReferenceList,
};
use eframe::egui::{self, Color32, PointerButton, Pos2, Rect, Stroke, Vec2};
use egui_material_icons::icons::{
    ICON_ARROW_BACK, ICON_CIRCLE, ICON_DATA_OBJECT, ICON_DIAGONAL_LINE, ICON_DRAW,
    ICON_FORMAT_COLOR_RESET, ICON_RECTANGLE, ICON_SELECT, ICON_TEXT_FIELDS, ICON_ZOOM_IN,
    ICON_ZOOM_OUT,
};
use uuid::Uuid;

use crate::block_picker::{BlockPicker, BlockPickerMenuAction};

use super::{
    clipboard::{ClipboardImagePaste, ClipboardImagePasteResult},
    image::{create_image_block, pick_image_file},
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorRegistration, SidebarDragPayload,
};

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: InfiniteCanvas::TYPE_ID,
        display_name: "Canvas",
        icon: ICON_DRAW,
        create: Some(|client| {
            Box::new(InfiniteCanvasEditor::new(
                client.create_block(InfiniteCanvas::new()),
                client,
            ))
        }),
        open: |client, id| {
            Box::new(InfiniteCanvasEditor::new(
                client.get_block::<InfiniteCanvas>(id),
                client,
            ))
        },
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

const MIN_SIZE: f32 = 4.0;
const HIT_RADIUS: f32 = 7.0;
const HANDLE_RADIUS: f32 = 5.0;
const ROTATE_OFFSET: f32 = 28.0;
const ZOOM_STEP: f32 = 1.25;
const MAX_IMPORTED_IMAGE_SIZE: f32 = 600.0;
const IMPORT_CASCADE_OFFSET: f32 = 24.0;
const DIRECT_EDITOR_PADDING: f32 = 12.0;
const DIRECT_EDITOR_TITLE_HEIGHT: f32 = 28.0;
const DIRECT_EDITOR_TITLE_GAP: f32 = 8.0;

#[derive(Clone, Copy, Debug, PartialEq)]
enum CommonValue<T> {
    None,
    Mixed,
    Uniform(T),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tool {
    Select,
    Line,
    Rectangle,
    Text,
    Pen,
    Block,
}

#[derive(Clone, Copy)]
enum Alignment {
    Left,
    HorizontalCenter,
    Right,
    Top,
    VerticalCenter,
    Bottom,
}

#[derive(Clone, Copy, Debug)]
struct WorldRect {
    min: CanvasPoint,
    max: CanvasPoint,
}

#[derive(Clone, Copy, Debug)]
struct SelectionFrame {
    center: CanvasPoint,
    size: CanvasPoint,
    rotation: f32,
}

impl SelectionFrame {
    fn from_world_rect(bounds: WorldRect) -> Self {
        Self {
            center: bounds.center(),
            size: bounds.size(),
            rotation: 0.0,
        }
    }

    fn point(self, local: CanvasPoint) -> CanvasPoint {
        local_to_world(
            CanvasTransform::new(self.center, self.size, self.rotation),
            local,
        )
    }

    fn contains(self, point: CanvasPoint) -> bool {
        let local = world_to_local(
            CanvasTransform::new(self.center, self.size, self.rotation),
            point,
        );
        local.x.abs() <= 0.5 && local.y.abs() <= 0.5
    }

    fn local_bounds(self) -> WorldRect {
        WorldRect {
            min: CanvasPoint::new(-self.size.x * 0.5, -self.size.y * 0.5),
            max: CanvasPoint::new(self.size.x * 0.5, self.size.y * 0.5),
        }
    }
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

    fn contains(self, point: CanvasPoint) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
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
        duplicate: bool,
    },
    Resize {
        handle: ResizeHandle,
        frame: SelectionFrame,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        default_preserve_aspect_ratio: bool,
        force_preserve_aspect_ratio: bool,
        preserve_aspect_ratio: bool,
    },
    Rotate {
        frame: SelectionFrame,
        start_angle: f32,
        current: CanvasPoint,
        originals: Vec<CanvasEntity>,
        snap_angle: bool,
    },
}

pub(super) struct InfiniteCanvasEditor {
    block: BlockHandle<InfiniteCanvas>,
    tool: Tool,
    render_scale: f32,
    selection: HashSet<Uuid>,
    gesture: Option<Gesture>,
    picker: BlockPicker,
    armed_block: Option<CachedBlock>,
    armed_block_needs_parent: bool,
    pending_block_center: Option<CanvasPoint>,
    context_menu_position: Option<CanvasPoint>,
    context_menu_for_selection: bool,
    dependencies: ReferenceList,
    editing_text: Option<Uuid>,
    focus_text_requested: bool,
    image_import_error: Option<String>,
    pending_file_drop_position: Option<CanvasPoint>,
    clipboard_image_paste: ClipboardImagePaste,
    focused_editor: Option<Uuid>,
    viewport_center: CanvasPoint,
    fit_selection_requested: bool,
}

impl InfiniteCanvasEditor {
    pub(super) fn new(block: BlockHandle<InfiniteCanvas>, client: &BlockClient) -> Self {
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            tool: Tool::Select,
            render_scale: 1.0,
            selection: HashSet::new(),
            gesture: None,
            picker: BlockPicker::default(),
            armed_block: None,
            armed_block_needs_parent: false,
            pending_block_center: None,
            context_menu_position: None,
            context_menu_for_selection: false,
            dependencies,
            editing_text: None,
            focus_text_requested: false,
            image_import_error: None,
            pending_file_drop_position: None,
            clipboard_image_paste: ClipboardImagePaste::default(),
            focused_editor: None,
            viewport_center: CanvasPoint::default(),
            fit_selection_requested: false,
        }
    }

    fn record_update(&mut self, before: Vec<CanvasEntity>, after: Vec<CanvasEntity>, group: bool) {
        if before == after || after.is_empty() {
            return;
        }
        let operation = InfiniteCanvasOperation::Update { entities: after };
        if group {
            self.block.operate_grouped([operation]);
        } else {
            self.block.finish_history_group();
            self.block.operate(operation);
        }
    }

    fn record_action(&mut self, operation: InfiniteCanvasOperation) {
        self.block.finish_history_group();
        self.block.operate(operation);
    }

    fn world_to_screen(&self, point: CanvasPoint, rect: Rect) -> Pos2 {
        rect.center() + Vec2::new(point.x, point.y) * self.render_scale
    }

    fn screen_to_world(&self, point: Pos2, rect: Rect) -> CanvasPoint {
        let relative = point - rect.center();
        CanvasPoint::new(
            relative.x / self.render_scale,
            relative.y / self.render_scale,
        )
    }

    fn selected_entities(&self, entities: &[CanvasEntity]) -> Vec<CanvasEntity> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .cloned()
            .collect()
    }

    fn selected_unlocked_entities(&self, entities: &[CanvasEntity]) -> Vec<CanvasEntity> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .cloned()
            .collect()
    }

    fn selection_has_unlocked(&self, entities: &[CanvasEntity]) -> bool {
        entities
            .iter()
            .any(|entity| self.selection.contains(&entity.id) && !entity.locked)
    }

    fn entity_selection_ids(entities: &[CanvasEntity], id: Uuid) -> HashSet<Uuid> {
        let group_id = entities
            .iter()
            .find(|entity| entity.id == id)
            .and_then(|entity| entity.group_id);
        entities
            .iter()
            .filter(|entity| match group_id {
                Some(group_id) => entity.group_id == Some(group_id),
                None => entity.id == id,
            })
            .map(|entity| entity.id)
            .collect()
    }

    fn select_entity(&mut self, entities: &[CanvasEntity], id: Uuid, additive: bool) {
        let ids = Self::entity_selection_ids(entities, id);
        if additive {
            if ids.iter().all(|id| self.selection.contains(id)) {
                self.selection.retain(|id| !ids.contains(id));
            } else {
                self.selection.extend(ids);
            }
        } else {
            if !ids.iter().all(|id| self.selection.contains(id)) {
                self.selection.clear();
            }
            self.selection.extend(ids);
        }
    }

    fn selected_bounds(&self, entities: &[CanvasEntity]) -> Option<WorldRect> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    fn selected_frame(&self, entities: &[CanvasEntity]) -> Option<SelectionFrame> {
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .collect::<Vec<_>>();
        match selected.as_slice() {
            [] => None,
            [entity] => Some(SelectionFrame {
                center: entity.transform.center,
                size: entity.transform.size,
                rotation: entity.transform.rotation,
            }),
            _ => selected
                .into_iter()
                .map(entity_bounds)
                .reduce(WorldRect::union)
                .map(SelectionFrame::from_world_rect),
        }
    }

    fn entity_at(&self, entities: &[CanvasEntity], point: CanvasPoint) -> Option<Uuid> {
        entities
            .iter()
            .rev()
            .find(|entity| hit_entity(entity, point, HIT_RADIUS / self.render_scale))
            .map(|entity| entity.id)
    }

    fn add_entity(&mut self, entity: CanvasEntity) {
        let id = entity.id;
        if matches!(entity.kind, CanvasEntityKind::Text { .. }) {
            self.editing_text = Some(id);
            self.focus_text_requested = true;
        }
        self.record_action(InfiniteCanvasOperation::Add { entity });
        self.selection.clear();
        self.selection.insert(id);
    }

    fn duplicate_selection(&mut self, entities: &[CanvasEntity]) {
        let duplicates = duplicate_entities(
            self.selected_entities(entities),
            CanvasPoint::new(IMPORT_CASCADE_OFFSET, IMPORT_CASCADE_OFFSET),
        );
        if duplicates.is_empty() {
            return;
        }
        self.block.finish_history_group();
        self.selection.clear();
        for entity in duplicates {
            self.selection.insert(entity.id);
            self.block.operate(InfiniteCanvasOperation::Add { entity });
        }
    }

    fn begin_move_gesture(
        &mut self,
        entities: &[CanvasEntity],
        world: CanvasPoint,
        duplicate: bool,
    ) {
        let mut originals = self.selected_unlocked_entities(entities);
        if duplicate {
            originals = duplicate_entities(originals, CanvasPoint::default());
            self.selection = originals.iter().map(|entity| entity.id).collect();
        }
        self.gesture = (!originals.is_empty()).then_some(Gesture::Move {
            start: world,
            current: world,
            originals,
            duplicate,
        });
    }

    fn group_selection(&mut self, entities: &[CanvasEntity]) {
        let before = self.selected_entities(entities);
        if before.len() < 2 {
            return;
        }
        let group_id = Uuid::new_v4();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.group_id = Some(group_id);
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    fn ungroup_selection(&mut self, entities: &[CanvasEntity]) {
        let groups = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .filter_map(|entity| entity.group_id)
            .collect::<HashSet<_>>();
        let before = entities
            .iter()
            .filter(|entity| entity.group_id.is_some_and(|group| groups.contains(&group)))
            .cloned()
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.group_id = None;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    fn set_selection_locked(&mut self, entities: &[CanvasEntity], locked: bool) {
        let before = self.selected_entities(entities);
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                entity.locked = locked;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    fn align_selection(&mut self, entities: &[CanvasEntity], alignment: Alignment) {
        let before = self.selected_unlocked_entities(entities);
        let Some(bounds) = before.iter().map(entity_bounds).reduce(WorldRect::union) else {
            return;
        };
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                let entity_bounds = entity_bounds(&entity);
                let delta = match alignment {
                    Alignment::Left => CanvasPoint::new(bounds.min.x - entity_bounds.min.x, 0.0),
                    Alignment::HorizontalCenter => {
                        CanvasPoint::new(bounds.center().x - entity_bounds.center().x, 0.0)
                    }
                    Alignment::Right => CanvasPoint::new(bounds.max.x - entity_bounds.max.x, 0.0),
                    Alignment::Top => CanvasPoint::new(0.0, bounds.min.y - entity_bounds.min.y),
                    Alignment::VerticalCenter => {
                        CanvasPoint::new(0.0, bounds.center().y - entity_bounds.center().y)
                    }
                    Alignment::Bottom => CanvasPoint::new(0.0, bounds.max.y - entity_bounds.max.y),
                };
                entity.transform.center.x += delta.x;
                entity.transform.center.y += delta.y;
                entity
            })
            .collect();
        self.record_update(before, after, false);
    }

    fn distribute_selection(&mut self, entities: &[CanvasEntity], horizontal: bool) {
        let before = self.selected_unlocked_entities(entities);
        if before.len() < 3 {
            return;
        }
        let mut ordered = before.clone();
        ordered.sort_by(|a, b| {
            let a = if horizontal {
                entity_bounds(a).center().x
            } else {
                entity_bounds(a).center().y
            };
            let b = if horizontal {
                entity_bounds(b).center().x
            } else {
                entity_bounds(b).center().y
            };
            a.total_cmp(&b)
        });
        let coordinate = |entity: &CanvasEntity| {
            let center = entity_bounds(entity).center();
            if horizontal {
                center.x
            } else {
                center.y
            }
        };
        let start = coordinate(&ordered[0]);
        let end = coordinate(ordered.last().unwrap());
        let step = (end - start) / (ordered.len() - 1) as f32;
        for (index, entity) in ordered.iter_mut().enumerate() {
            let delta = start + step * index as f32 - coordinate(entity);
            if horizontal {
                entity.transform.center.x += delta;
            } else {
                entity.transform.center.y += delta;
            }
        }
        self.record_update(before, ordered, false);
    }

    fn add_direct_editor(&mut self, block_id: Uuid, center: CanvasPoint) {
        self.add_direct_editor_sized(block_id, center, CanvasPoint::new(180.0, 100.0));
    }

    fn add_direct_editor_sized(
        &mut self,
        block_id: Uuid,
        center: CanvasPoint,
        content_size: CanvasPoint,
    ) {
        let size = direct_editor_entity_size(Vec2::new(content_size.x, content_size.y), 1.0);
        self.add_entity(CanvasEntity {
            id: Uuid::new_v4(),
            transform: CanvasTransform::new(center, size, 0.0),
            kind: CanvasEntityKind::DirectEditor {
                block_id,
                scale: 1.0,
            },
            style: CanvasEntityStyle::default(),
            group_id: None,
            locked: false,
        });
    }

    fn imported_image_size(image: &ImageBlock) -> CanvasPoint {
        let width = image.width() as f32;
        let height = image.height() as f32;
        let scale = (MAX_IMPORTED_IMAGE_SIZE / width.max(height)).min(1.0);
        CanvasPoint::new(width * scale, height * scale)
    }

    fn add_imported_image(
        &mut self,
        editors: &mut EditorAccess<'_>,
        image: ImageBlock,
        center: CanvasPoint,
    ) {
        let size = Self::imported_image_size(&image);
        let id = create_image_block(editors, image, self.block.id());
        self.add_direct_editor_sized(id, center, size);
    }

    fn ensure_dependency_editors(
        entities: &[CanvasEntity],
        dependencies: &[block::BlockReference],
        editors: &mut EditorAccess<'_>,
    ) {
        let referenced = entities
            .iter()
            .filter_map(|entity| match entity.kind {
                CanvasEntityKind::Block { block_id }
                | CanvasEntityKind::DirectEditor { block_id, .. } => Some(block_id),
                _ => None,
            })
            .collect::<HashSet<_>>();
        for dependency in dependencies {
            if referenced.contains(&dependency.id) {
                editors.ensure(dependency.id, dependency.block_type);
            }
        }
    }

    fn selection_defaults_to_proportional(
        &self,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) -> bool {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .any(|entity| match entity.kind {
                CanvasEntityKind::Block { block_id } => {
                    editors.default_preserve_aspect_ratio(block_id)
                }
                CanvasEntityKind::DirectEditor { block_id, .. } => editors
                    .direct_editor_capabilities(block_id)
                    .is_some_and(|capabilities| capabilities.preserve_aspect_ratio),
                _ => false,
            })
    }

    fn selection_allows_rotation(
        &self,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) -> bool {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .all(|entity| match entity.kind {
                CanvasEntityKind::DirectEditor { block_id, .. } => editors
                    .direct_editor_capabilities(block_id)
                    .is_none_or(|capabilities| capabilities.allow_rotation),
                _ => true,
            })
    }

    fn selection_forces_proportional(
        &self,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) -> bool {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .any(|entity| match entity.kind {
                CanvasEntityKind::DirectEditor { block_id, .. } => editors
                    .direct_editor_capabilities(block_id)
                    .is_some_and(|capabilities| capabilities.preserve_aspect_ratio),
                _ => false,
            })
    }

    fn autosize_direct_editors(
        &mut self,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
    ) {
        let mut before = Vec::new();
        let mut after = Vec::new();
        for entity in entities {
            let CanvasEntityKind::DirectEditor { block_id, scale } = entity.kind else {
                continue;
            };
            let Some(intrinsic) = editors.direct_editor_intrinsic_size(block_id) else {
                continue;
            };
            let desired = direct_editor_entity_size(intrinsic, scale);
            if (entity.transform.size.x - desired.x).abs() < 0.01
                && (entity.transform.size.y - desired.y).abs() < 0.01
                && entity.transform.rotation == 0.0
            {
                continue;
            }
            let bounds = entity_bounds(entity);
            let mut updated = entity.clone();
            updated.transform.size = desired;
            updated.transform.rotation = 0.0;
            updated.transform.center = CanvasPoint::new(
                bounds.min.x + desired.x * 0.5,
                bounds.min.y + desired.y * 0.5,
            );
            before.push(entity.clone());
            after.push(updated);
        }
        self.record_update(before, after, true);
    }

    fn update_selected(
        &mut self,
        entities: &[CanvasEntity],
        compatible: impl Fn(&CanvasEntityKind) -> bool,
        mut update: impl FnMut(&mut CanvasEntityStyle),
    ) {
        let before = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && compatible(&entity.kind))
            .cloned()
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                update(&mut entity.style);
                entity
            })
            .collect::<Vec<_>>();
        self.record_update(before, after, true);
    }

    fn show_inspector(
        &mut self,
        ui: &mut egui::Ui,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
        show_heading: bool,
    ) -> Option<CanvasLayerMove> {
        if show_heading {
            ui.heading("Inspector");
            ui.separator();
        }
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            ui.weak("Select an object to edit its appearance.");
            return None;
        }

        ui.weak(match selected.len() {
            1 => "1 object selected".into(),
            count => format!("{count} objects selected"),
        });

        ui.separator();
        ui.strong("Transform");
        if let [entity] = selected.as_slice() {
            let transform_enabled = !entity.locked;
            let mut updated = (*entity).clone();
            let mut changed = false;
            egui::Grid::new("canvas-transform-fields")
                .num_columns(4)
                .show(ui, |ui| {
                    ui.label("X");
                    changed |= ui
                        .add_enabled(
                            transform_enabled,
                            egui::DragValue::new(&mut updated.transform.center.x).speed(1.0),
                        )
                        .changed();
                    ui.label("Y");
                    changed |= ui
                        .add_enabled(
                            transform_enabled,
                            egui::DragValue::new(&mut updated.transform.center.y).speed(1.0),
                        )
                        .changed();
                    ui.end_row();

                    let original_size = updated.transform.size;
                    let mut width = original_size.x;
                    let mut height = original_size.y;
                    ui.label("W");
                    let width_changed = ui
                        .add_enabled(
                            transform_enabled,
                            egui::DragValue::new(&mut width)
                                .speed(1.0)
                                .range(MIN_SIZE..=f32::INFINITY),
                        )
                        .changed();
                    ui.label("H");
                    let height_changed = ui
                        .add_enabled(
                            transform_enabled,
                            egui::DragValue::new(&mut height)
                                .speed(1.0)
                                .range(MIN_SIZE..=f32::INFINITY),
                        )
                        .changed();
                    ui.end_row();
                    if let CanvasEntityKind::DirectEditor { scale, .. } = &mut updated.kind {
                        let factor = if width_changed {
                            width / original_size.x.max(MIN_SIZE)
                        } else if height_changed {
                            height / original_size.y.max(MIN_SIZE)
                        } else {
                            1.0
                        };
                        if width_changed || height_changed {
                            updated.transform.size.x = original_size.x * factor;
                            updated.transform.size.y = original_size.y * factor;
                            *scale *= factor;
                            changed = true;
                        }
                    } else {
                        if width_changed {
                            updated.transform.size.x = width;
                            changed = true;
                        }
                        if height_changed {
                            updated.transform.size.y = height;
                            changed = true;
                        }
                    }

                    let mut degrees = updated.transform.rotation.to_degrees();
                    ui.label("Rotation");
                    let rotation = ui.add_enabled(
                        transform_enabled && self.selection_allows_rotation(entities, editors),
                        egui::DragValue::new(&mut degrees).speed(1.0).suffix("°"),
                    );
                    if rotation.changed() {
                        updated.transform.rotation = degrees.to_radians();
                        changed = true;
                    }
                    ui.end_row();
                });
            if changed {
                self.record_update(vec![(*entity).clone()], vec![updated], true);
            }
        } else {
            ui.weak("Select one object to edit exact transform values.");
        }

        if let [entity] = selected.as_slice() {
            let block_mode = match entity.kind {
                CanvasEntityKind::Block { block_id } => Some((block_id, false)),
                CanvasEntityKind::DirectEditor { block_id, .. } => Some((block_id, true)),
                _ => None,
            };
            if let Some((block_id, direct)) = block_mode {
                ui.separator();
                ui.strong("Block");
                let available = direct || editors.direct_editor_capabilities(block_id).is_some();
                let label = if direct {
                    "Show preview only"
                } else {
                    "Use direct editor"
                };
                if ui
                    .add_enabled(available, egui::Button::new(label))
                    .on_disabled_hover_text("Waiting for the block editor to load")
                    .clicked()
                {
                    let updated = if direct {
                        if self.focused_editor == Some(entity.id) {
                            self.focused_editor = None;
                        }
                        direct_editor_to_preview(entity, block_id)
                    } else {
                        editors
                            .direct_editor_intrinsic_size(block_id)
                            .map(|intrinsic| preview_to_direct_editor(entity, block_id, intrinsic))
                    };
                    if let Some(updated) = updated {
                        self.record_update(vec![(*entity).clone()], vec![updated], true);
                    }
                }
            }
        }

        let foreground = common_value(
            selected
                .iter()
                .filter(|entity| {
                    !matches!(
                        entity.kind,
                        CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
                    )
                })
                .map(|entity| entity.style.foreground),
        );
        if !matches!(foreground, CommonValue::None) {
            if let Some(color) = color_menu(ui, "Color", foreground) {
                self.update_selected(
                    entities,
                    |kind| {
                        !matches!(
                            kind,
                            CanvasEntityKind::Block { .. } | CanvasEntityKind::DirectEditor { .. }
                        )
                    },
                    |style| style.foreground = color,
                );
            }
        }

        let stroked = selected.iter().copied().filter(|entity| {
            matches!(
                entity.kind,
                CanvasEntityKind::Line | CanvasEntityKind::Rectangle | CanvasEntityKind::Pen { .. }
            )
        });
        let width = common_value(stroked.map(|entity| entity.style.line_width));
        if !matches!(width, CommonValue::None) {
            let mixed = matches!(width, CommonValue::Mixed);
            let mut value = match width {
                CommonValue::Uniform(value) => value,
                CommonValue::Mixed | CommonValue::None => 2.0,
            };
            ui.horizontal(|ui| {
                ui.label("Line width");
                if mixed {
                    ui.weak("Mixed");
                }
            });
            if ui
                .add(egui::Slider::new(&mut value, 0.5..=20.0).suffix(" px"))
                .changed()
            {
                self.update_selected(
                    entities,
                    |kind| {
                        matches!(
                            kind,
                            CanvasEntityKind::Line
                                | CanvasEntityKind::Rectangle
                                | CanvasEntityKind::Pen { .. }
                        )
                    },
                    |style| style.line_width = value,
                );
            }
        }

        let lines = selected
            .iter()
            .copied()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Line))
            .collect::<Vec<_>>();
        if !lines.is_empty() {
            ui.separator();
            ui.strong("Line");
            for (label, value, set) in [
                (
                    "Dashed",
                    common_value(lines.iter().map(|entity| entity.style.dashed)),
                    0_u8,
                ),
                (
                    "Start arrow",
                    common_value(lines.iter().map(|entity| entity.style.arrow_start)),
                    1,
                ),
                (
                    "End arrow",
                    common_value(lines.iter().map(|entity| entity.style.arrow_end)),
                    2,
                ),
            ] {
                if let Some(value) = mixed_checkbox(ui, label, value) {
                    self.update_selected(
                        entities,
                        |kind| matches!(kind, CanvasEntityKind::Line),
                        |style| match set {
                            0 => style.dashed = value,
                            1 => style.arrow_start = value,
                            _ => style.arrow_end = value,
                        },
                    );
                }
            }
        }

        let rectangles = selected
            .iter()
            .copied()
            .filter(|entity| matches!(entity.kind, CanvasEntityKind::Rectangle))
            .collect::<Vec<_>>();
        if !rectangles.is_empty() {
            ui.separator();
            ui.strong("Rectangle");
            let fill = common_value(rectangles.iter().map(|entity| entity.style.fill));
            if let Some(fill) = fill_color_menu(ui, fill) {
                self.update_selected(
                    entities,
                    |kind| matches!(kind, CanvasEntityKind::Rectangle),
                    |style| style.fill = fill,
                );
            }

            let radius = common_value(rectangles.iter().map(|entity| entity.style.corner_radius));
            let mixed = matches!(radius, CommonValue::Mixed);
            let mut value = match radius {
                CommonValue::Uniform(value) => value,
                CommonValue::Mixed | CommonValue::None => 0.0,
            };
            ui.horizontal(|ui| {
                ui.label("Corner radius");
                if mixed {
                    ui.weak("Mixed");
                }
            });
            if ui
                .add(egui::Slider::new(&mut value, 0.0..=100.0).suffix(" px"))
                .changed()
            {
                self.update_selected(
                    entities,
                    |kind| matches!(kind, CanvasEntityKind::Rectangle),
                    |style| style.corner_radius = value,
                );
            }
        }

        ui.separator();
        let opacity = common_value(selected.iter().map(|entity| entity.style.opacity));
        let mixed = matches!(opacity, CommonValue::Mixed);
        let mut value = match opacity {
            CommonValue::Uniform(value) => value,
            CommonValue::Mixed | CommonValue::None => 1.0,
        };
        ui.horizontal(|ui| {
            ui.label("Opacity");
            if mixed {
                ui.weak("Mixed");
            }
        });
        if ui
            .add(
                egui::Slider::new(&mut value, 0.0..=1.0)
                    .custom_formatter(|value, _| format!("{:.0}%", value * 100.0)),
            )
            .changed()
        {
            self.update_selected(entities, |_| true, |style| style.opacity = value);
        }

        ui.separator();
        ui.strong("Arrange");
        let movable_count = selected.iter().filter(|entity| !entity.locked).count();
        ui.horizontal_wrapped(|ui| {
            for (label, alignment) in [
                ("Left", Alignment::Left),
                ("Center", Alignment::HorizontalCenter),
                ("Right", Alignment::Right),
                ("Top", Alignment::Top),
                ("Middle", Alignment::VerticalCenter),
                ("Bottom", Alignment::Bottom),
            ] {
                if ui
                    .add_enabled(movable_count >= 2, egui::Button::new(label).small())
                    .clicked()
                {
                    self.align_selection(entities, alignment);
                }
            }
        });
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    movable_count >= 3,
                    egui::Button::new("Distribute horizontally").small(),
                )
                .clicked()
            {
                self.distribute_selection(entities, true);
            }
            if ui
                .add_enabled(
                    movable_count >= 3,
                    egui::Button::new("Distribute vertically").small(),
                )
                .clicked()
            {
                self.distribute_selection(entities, false);
            }
        });
        let mut movement = None;
        ui.columns(2, |columns| {
            if columns[0].button("To front").clicked() {
                movement = Some(CanvasLayerMove::BringToFront);
            }
            if columns[1].button("Forward").clicked() {
                movement = Some(CanvasLayerMove::ForwardOne);
            }
            if columns[0].button("Backward").clicked() {
                movement = Some(CanvasLayerMove::BackOne);
            }
            if columns[1].button("To back").clicked() {
                movement = Some(CanvasLayerMove::SendToBack);
            }
        });
        let can_group = selected.len() >= 2;
        let can_ungroup = selected.iter().any(|entity| entity.group_id.is_some());
        ui.columns(2, |columns| {
            if columns[0]
                .add_enabled(can_group, egui::Button::new("Group"))
                .clicked()
            {
                self.group_selection(entities);
            }
            if columns[1]
                .add_enabled(can_ungroup, egui::Button::new("Ungroup"))
                .clicked()
            {
                self.ungroup_selection(entities);
            }
        });
        let can_lock = selected.iter().any(|entity| !entity.locked);
        let can_unlock = selected.iter().any(|entity| entity.locked);
        ui.columns(2, |columns| {
            if columns[0]
                .add_enabled(can_lock, egui::Button::new("Lock"))
                .clicked()
            {
                self.set_selection_locked(entities, true);
            }
            if columns[1]
                .add_enabled(can_unlock, egui::Button::new("Unlock"))
                .clicked()
            {
                self.set_selection_locked(entities, false);
            }
        });
        movement
    }

    fn show_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        _entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<Uuid> {
        let mut create_block = None;
        ui.horizontal_wrapped(|ui| {
            for (tool, icon, label) in [
                (Tool::Select, ICON_SELECT, "Select"),
                (Tool::Line, ICON_DIAGONAL_LINE, "Line"),
                (Tool::Rectangle, ICON_RECTANGLE, "Rectangle"),
                (Tool::Text, ICON_TEXT_FIELDS, "Text"),
                (Tool::Pen, ICON_DRAW, "Pen"),
            ] {
                if ui
                    .selectable_label(self.tool == tool, icon)
                    .on_hover_text(label)
                    .clicked()
                {
                    self.tool = tool;
                    self.armed_block = None;
                    self.armed_block_needs_parent = false;
                }
            }
            ui.menu_button(ICON_DATA_OBJECT, |ui| {
                if let Some(action) = BlockPicker::show_menu(ui, editors.registry()) {
                    self.tool = Tool::Block;
                    self.armed_block = None;
                    self.armed_block_needs_parent = false;
                    self.pending_block_center = Some(self.viewport_center);
                    match action {
                        BlockPickerMenuAction::New(block_type) => {
                            create_block = Some(block_type);
                        }
                        BlockPickerMenuAction::ImportImage => match pick_image_file() {
                            Ok(Some(image)) => {
                                self.image_import_error = None;
                                self.add_imported_image(editors, image, self.viewport_center);
                                self.tool = Tool::Select;
                            }
                            Ok(None) => {}
                            Err(error) => self.image_import_error = Some(error),
                        },
                        BlockPickerMenuAction::LinkExisting => {
                            self.picker.open([self.block.id()]);
                        }
                    }
                }
            })
            .response
            .on_hover_text("Block");

            ui.separator();
            if ui
                .small_button(ICON_ZOOM_OUT)
                .on_hover_text("Zoom out")
                .clicked()
            {
                viewport.change_zoom(1.0 / ZOOM_STEP, None);
            }
            ui.menu_button(format!("{:.0}%", viewport.zoom() * 100.0), |ui| {
                for percent in [25.0, 50.0, 100.0, 200.0] {
                    if ui.button(format!("{percent:.0}%")).clicked() {
                        viewport.change_zoom(percent / 100.0 / viewport.zoom(), None);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Fit all").clicked() {
                    viewport.fit();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        !self.selection.is_empty(),
                        egui::Button::new("Fit selection"),
                    )
                    .clicked()
                {
                    self.fit_selection_requested = true;
                    ui.close();
                }
            })
            .response
            .on_hover_text("Zoom presets and fit controls");
            if ui
                .small_button(ICON_ZOOM_IN)
                .on_hover_text("Zoom in")
                .clicked()
            {
                viewport.change_zoom(ZOOM_STEP, None);
            }

            ui.menu_button("?", |ui| {
                ui.strong("Canvas shortcuts");
                egui::Grid::new("canvas-shortcuts").show(ui, |ui| {
                    for (action, shortcut) in [
                        ("Select / Rectangle / Line", "V / R / L"),
                        ("Text / Pen", "T / P"),
                        ("Pan", "Space-drag or middle-drag"),
                        ("Zoom", "Ctrl/Cmd-scroll or pinch"),
                        ("Select all", "Ctrl/Cmd+A"),
                        ("Nudge", "Arrow keys; Shift for 10×"),
                        ("Duplicate", "Ctrl/Cmd+D or Alt-drag"),
                        ("Edit selected block", "Enter"),
                        ("Exit tool or editor", "Escape"),
                    ] {
                        ui.label(action);
                        ui.weak(shortcut);
                        ui.end_row();
                    }
                });
            })
            .response
            .on_hover_text("Canvas help and shortcuts");

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
        });
        ui.separator();
        create_block
    }

    fn show_picker(&mut self, context: &egui::Context, client: &BlockClient) {
        if let Some(block) = self.picker.show(context, client) {
            if let Some(center) = self.pending_block_center.take() {
                self.add_direct_editor(block.id, center);
                self.tool = Tool::Select;
            } else {
                self.armed_block = Some(block);
                self.armed_block_needs_parent = false;
                self.tool = Tool::Block;
            }
        }
    }

    fn import_dropped_images(&mut self, response: &egui::Response, editors: &mut EditorAccess<'_>) {
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
                self.pending_file_drop_position =
                    Some(self.screen_to_world(position, response.rect));
            }
        }
        if dropped.is_empty() {
            if !hovering_file {
                self.pending_file_drop_position = None;
            }
            return;
        }
        self.image_import_error = None;
        let base = self
            .pending_file_drop_position
            .take()
            .or_else(|| {
                response
                    .ctx
                    .pointer_hover_pos()
                    .filter(|position| response.rect.contains(*position))
                    .map(|position| self.screen_to_world(position, response.rect))
            })
            .unwrap_or_else(|| self.screen_to_world(response.rect.center(), response.rect));
        for (index, file) in dropped.into_iter().enumerate() {
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
                        self.image_import_error =
                            Some(format!("Could not read {source_name}: {error}"));
                        continue;
                    }
                    None => {
                        self.image_import_error =
                            Some(format!("No image data was available for {source_name}"));
                        continue;
                    }
                },
            };
            match ImageBlock::from_compressed(source_name.clone(), bytes) {
                Ok(image) => {
                    let offset = IMPORT_CASCADE_OFFSET * index as f32;
                    self.add_imported_image(
                        editors,
                        image,
                        CanvasPoint::new(base.x + offset, base.y + offset),
                    );
                }
                Err(error) => {
                    self.image_import_error =
                        Some(format!("Could not import {source_name}: {error}"));
                }
            }
        }
    }

    fn import_clipboard_image(
        &mut self,
        response: &egui::Response,
        editors: &mut EditorAccess<'_>,
    ) {
        let enabled = !response.ctx.egui_wants_keyboard_input();
        let Some(result) = self.clipboard_image_paste.poll(&response.ctx, enabled) else {
            return;
        };
        let ClipboardImagePasteResult::Image(image) = result else {
            if let ClipboardImagePasteResult::Error(error) = result {
                self.image_import_error = Some(error);
            }
            return;
        };
        self.image_import_error = None;
        let screen_position = response
            .ctx
            .pointer_hover_pos()
            .filter(|position| response.rect.contains(*position))
            .unwrap_or_else(|| response.rect.center());
        let center = self.screen_to_world(screen_position, response.rect);
        self.add_imported_image(editors, image, center);
    }

    fn show_inline_text_editor(
        &mut self,
        ui: &mut egui::Ui,
        canvas_rect: Rect,
        entities: &[CanvasEntity],
    ) {
        let Some(id) = self.editing_text else {
            return;
        };
        let Some(entity) = entities.iter().find(|entity| entity.id == id).cloned() else {
            self.editing_text = None;
            self.focus_text_requested = false;
            return;
        };
        let CanvasEntityKind::Text { text, text_style } = &entity.kind else {
            self.editing_text = None;
            self.focus_text_requested = false;
            return;
        };
        if entity.locked {
            self.editing_text = None;
            self.focus_text_requested = false;
            return;
        }

        let bounds = screen_rect(self, entity_bounds(&entity), canvas_rect);
        let edit_rect =
            Rect::from_center_size(bounds.center(), bounds.size().max(Vec2::new(80.0, 36.0)));
        let mut edited = text.clone();
        let font_size = (text_style.font_size * self.render_scale).clamp(8.0, 256.0);
        let output = ui
            .new_child(
                egui::UiBuilder::new()
                    .id_salt(("canvas-inline-text", id))
                    .max_rect(edit_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
            .scope(|ui| {
                ui.set_min_size(edit_rect.size());
                ui.set_max_size(edit_rect.size());
                egui::TextEdit::multiline(&mut edited)
                    .font(egui::FontId::proportional(font_size))
                    .desired_width(edit_rect.width())
                    .desired_rows(1)
                    .show(ui)
            })
            .inner;
        let response = &output.response.response;
        let requested_focus = std::mem::take(&mut self.focus_text_requested);
        if requested_focus {
            response.request_focus();
        }

        if response.changed() {
            let mut updated = entity.clone();
            updated.kind = CanvasEntityKind::Text {
                text: edited,
                text_style: *text_style,
            };
            if !text_style.wrap {
                updated.transform.size = CanvasPoint::new(
                    (output.galley.size().x / self.render_scale + 8.0).max(16.0),
                    (output.galley.size().y / self.render_scale + 8.0)
                        .max(text_style.font_size * text_style.line_height),
                );
            }
            self.record_update(vec![entity], vec![updated], true);
        }

        let exit = ui.ctx().input(|input| {
            input.key_pressed(egui::Key::Escape)
                || (input.modifiers.command && input.key_pressed(egui::Key::Enter))
        });
        if exit || (!requested_focus && response.lost_focus()) {
            response.surrender_focus();
            self.editing_text = None;
            self.block.finish_history_group();
        }
    }

    fn handle_zoom_and_pan(
        &mut self,
        response: &egui::Response,
        viewport: &mut DirectEditorViewport,
    ) -> bool {
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

        let panning = response.ctx.input(|input| {
            input.pointer.button_down(PointerButton::Middle)
                || (input.key_down(egui::Key::Space)
                    && input.pointer.button_down(PointerButton::Primary))
        });
        if panning && response.hovered() {
            response.ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            let delta = response.ctx.input(|input| input.pointer.delta());
            viewport.pan(delta);
            self.gesture = None;
            return true;
        }
        false
    }

    fn update_cursor(
        &self,
        response: &egui::Response,
        world: CanvasPoint,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) {
        if response.ctx.input(|input| input.key_down(egui::Key::Space)) {
            response.ctx.set_cursor_icon(egui::CursorIcon::Grab);
            return;
        }
        let cursor = match self.tool {
            Tool::Line | Tool::Rectangle | Tool::Pen => egui::CursorIcon::Crosshair,
            Tool::Text => egui::CursorIcon::Text,
            Tool::Block => egui::CursorIcon::Copy,
            Tool::Select => {
                let frame = self.selected_frame(entities);
                if self.selection_has_unlocked(entities)
                    && self.selection_allows_rotation(entities, editors)
                    && frame.is_some_and(|frame| {
                        rotate_handle_at(self, frame, response.rect)
                            .distance(self.world_to_screen(world, response.rect))
                            <= HANDLE_RADIUS + 3.0
                    })
                {
                    egui::CursorIcon::Grab
                } else if let Some(handle) = self
                    .selection_has_unlocked(entities)
                    .then(|| frame)
                    .flatten()
                    .and_then(|frame| resize_handle_at(self, frame, response.rect, world))
                {
                    match (handle.x, handle.y) {
                        (0, _) => egui::CursorIcon::ResizeVertical,
                        (_, 0) => egui::CursorIcon::ResizeHorizontal,
                        (x, y) if x == y => egui::CursorIcon::ResizeNwSe,
                        _ => egui::CursorIcon::ResizeNeSw,
                    }
                } else {
                    egui::CursorIcon::Default
                }
            }
        };
        response.ctx.set_cursor_icon(cursor);
    }

    fn handle_canvas_input(
        &mut self,
        response: &egui::Response,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
        direct_editor_rects: &[Rect],
        viewport: &mut DirectEditorViewport,
    ) -> (
        Option<CanvasLayerMove>,
        Option<Uuid>,
        Option<Uuid>,
        Option<EditorAction>,
    ) {
        let escape_pressed = response
            .ctx
            .input(|input| input.key_pressed(egui::Key::Escape));
        if escape_pressed
            && self.focused_editor.is_some()
            && !response.ctx.egui_wants_keyboard_input()
        {
            self.focused_editor = None;
        } else if escape_pressed {
            self.gesture = None;
            self.armed_block = None;
            self.armed_block_needs_parent = false;
            self.picker.close();
            self.tool = Tool::Select;
        }
        let keyboard_available = !response.ctx.egui_wants_keyboard_input();
        let mut layer_move = None;
        let mut keyboard_action = None;
        if self.focused_editor.is_none() && keyboard_available {
            let modifiers = response.ctx.input(|input| input.modifiers);
            if !modifiers.command && !modifiers.alt {
                for (key, tool) in [
                    (egui::Key::V, Tool::Select),
                    (egui::Key::R, Tool::Rectangle),
                    (egui::Key::L, Tool::Line),
                    (egui::Key::T, Tool::Text),
                    (egui::Key::P, Tool::Pen),
                ] {
                    if response.ctx.input(|input| input.key_pressed(key)) {
                        self.tool = tool;
                        self.gesture = None;
                        self.armed_block = None;
                        self.armed_block_needs_parent = false;
                    }
                }
            }
            if modifiers.command && response.ctx.input(|input| input.key_pressed(egui::Key::A)) {
                if modifiers.shift {
                    let selected = &self.selection;
                    self.selection = entities
                        .iter()
                        .filter(|entity| !selected.contains(&entity.id))
                        .map(|entity| entity.id)
                        .collect();
                } else {
                    self.selection = entities.iter().map(|entity| entity.id).collect();
                }
                self.tool = Tool::Select;
            }
            if modifiers.command && response.ctx.input(|input| input.key_pressed(egui::Key::D)) {
                self.duplicate_selection(entities);
            }
            if modifiers.command
                && response
                    .ctx
                    .input(|input| input.key_pressed(egui::Key::OpenBracket))
            {
                layer_move = Some(CanvasLayerMove::BackOne);
            }
            if modifiers.command
                && response
                    .ctx
                    .input(|input| input.key_pressed(egui::Key::CloseBracket))
            {
                layer_move = Some(CanvasLayerMove::ForwardOne);
            }
            let nudge = response.ctx.input(|input| {
                let amount = if input.modifiers.shift { 10.0 } else { 1.0 };
                if input.key_pressed(egui::Key::ArrowLeft) {
                    Some(CanvasPoint::new(-amount, 0.0))
                } else if input.key_pressed(egui::Key::ArrowRight) {
                    Some(CanvasPoint::new(amount, 0.0))
                } else if input.key_pressed(egui::Key::ArrowUp) {
                    Some(CanvasPoint::new(0.0, -amount))
                } else if input.key_pressed(egui::Key::ArrowDown) {
                    Some(CanvasPoint::new(0.0, amount))
                } else {
                    None
                }
            });
            if let Some(nudge) = nudge {
                let before = self.selected_unlocked_entities(entities);
                let after = before
                    .iter()
                    .cloned()
                    .map(|mut entity| {
                        entity.transform.center.x += nudge.x;
                        entity.transform.center.y += nudge.y;
                        entity
                    })
                    .collect();
                self.record_update(before, after, true);
            }
            if response
                .ctx
                .input(|input| input.key_pressed(egui::Key::Enter))
                && self.selection.len() == 1
            {
                if let Some(entity) = entities
                    .iter()
                    .find(|entity| self.selection.contains(&entity.id))
                {
                    match entity.kind {
                        CanvasEntityKind::DirectEditor { .. } => {
                            self.focused_editor = Some(entity.id);
                        }
                        CanvasEntityKind::Text { .. } if !entity.locked => {
                            self.editing_text = Some(entity.id);
                            self.focus_text_requested = true;
                        }
                        CanvasEntityKind::Block { block_id } => {
                            if let Some(cached) = editors.client().cached_block(block_id) {
                                keyboard_action = Some(EditorAction::OpenBlock {
                                    id: cached.id,
                                    block_type: cached.block_type,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        if self.focused_editor.is_none()
            && self.tool == Tool::Select
            && response.ctx.input(|input| {
                input.key_pressed(egui::Key::Delete) || input.key_pressed(egui::Key::Backspace)
            })
            && !self.selection.is_empty()
            && keyboard_available
        {
            let ids = entities
                .iter()
                .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
                .map(|entity| entity.id)
                .collect::<Vec<_>>();
            if !ids.is_empty() {
                let removed = ids.iter().copied().collect::<HashSet<_>>();
                self.selection.retain(|id| !removed.contains(id));
                self.record_action(InfiniteCanvasOperation::Remove { ids });
            }
        }

        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.ctx.pointer_hover_pos());
        if self.focused_editor.is_some()
            && pointer.is_some_and(|pointer| {
                direct_editor_rects
                    .iter()
                    .any(|rect| rect.contains(pointer))
            })
        {
            return (None, None, None, keyboard_action);
        }

        if self.handle_zoom_and_pan(response, viewport) {
            return (None, None, None, keyboard_action);
        }

        let world = pointer.map(|point| self.screen_to_world(point, response.rect));

        if response.hovered() {
            if let Some(world) = world {
                self.update_cursor(response, world, entities, editors);
            }
        }

        if let Some(dragged) = response.dnd_hover_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                response.ctx.set_cursor_icon(egui::CursorIcon::Alias);
            }
        }

        if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                if let Some(world) = world {
                    self.add_direct_editor(dragged.reference.id, world);
                }
            }
        }

        if response.secondary_clicked() {
            if let Some(world) = world {
                self.context_menu_position = Some(world);
                self.context_menu_for_selection = self
                    .selected_frame(entities)
                    .is_some_and(|frame| frame.contains(world));
                if self.selection.is_empty() {
                    if let Some(id) = self.entity_at(entities, world) {
                        self.select_entity(entities, id, false);
                        self.context_menu_for_selection = true;
                    }
                } else if !self.context_menu_for_selection {
                    self.selection.clear();
                }
            }
        }
        let mut create_block = None;
        let mut set_parent = None;
        response.context_menu(|ui| {
            if self.context_menu_for_selection {
                for (label, movement) in [
                    ("Bring to front", CanvasLayerMove::BringToFront),
                    ("Forward", CanvasLayerMove::ForwardOne),
                    ("Backward", CanvasLayerMove::BackOne),
                    ("Send to back", CanvasLayerMove::SendToBack),
                ] {
                    if ui.button(label).clicked() {
                        layer_move = Some(movement);
                        ui.close();
                    }
                }
            } else {
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
                                style: CanvasEntityStyle::default(),
                                group_id: None,
                                locked: false,
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
                                style: CanvasEntityStyle::default(),
                                group_id: None,
                                locked: false,
                            });
                        }
                        self.tool = Tool::Select;
                        ui.close();
                    }
                    ui.menu_button("Block", |ui| {
                        if let Some(action) = BlockPicker::show_menu(ui, editors.registry()) {
                            self.tool = Tool::Block;
                            self.armed_block = None;
                            self.armed_block_needs_parent = false;
                            self.pending_block_center = self.context_menu_position;
                            match action {
                                BlockPickerMenuAction::New(block_type) => {
                                    create_block = Some(block_type);
                                }
                                BlockPickerMenuAction::ImportImage => match pick_image_file() {
                                    Ok(Some(image)) => {
                                        self.image_import_error = None;
                                        let center = self.context_menu_position.unwrap_or_default();
                                        self.add_imported_image(editors, image, center);
                                        self.tool = Tool::Select;
                                    }
                                    Ok(None) => {}
                                    Err(error) => self.image_import_error = Some(error),
                                },
                                BlockPickerMenuAction::LinkExisting => {
                                    self.picker.open([self.block.id()]);
                                }
                            }
                        }
                    });
                });
            }
        });

        let Some(world) = world else {
            return (layer_move, create_block, set_parent, keyboard_action);
        };
        let primary_pressed = response
            .ctx
            .input(|input| input.pointer.button_pressed(PointerButton::Primary));
        if primary_pressed
            && response.hovered()
            && self.focused_editor.is_some_and(|focused| {
                entities
                    .iter()
                    .find(|entity| entity.id == focused)
                    .is_none_or(|entity| !entity_bounds(entity).contains(world))
            })
        {
            self.focused_editor = None;
        }
        if primary_pressed && response.hovered() {
            match self.tool {
                Tool::Select => {
                    let selected_frame = self.selected_frame(entities);
                    let has_unlocked = self.selection_has_unlocked(entities);
                    let handle = has_unlocked
                        .then(|| selected_frame)
                        .flatten()
                        .and_then(|frame| resize_handle_at(self, frame, response.rect, world));
                    let rotate = has_unlocked
                        && self.selection_allows_rotation(entities, editors)
                        && selected_frame.is_some_and(|frame| {
                            rotate_handle_at(self, frame, response.rect)
                                .distance(self.world_to_screen(world, response.rect))
                                <= HANDLE_RADIUS + 3.0
                        });
                    if rotate {
                        let frame = selected_frame.unwrap();
                        let center = frame.center;
                        self.gesture = Some(Gesture::Rotate {
                            frame,
                            start_angle: (world.y - center.y).atan2(world.x - center.x),
                            current: world,
                            originals: self.selected_unlocked_entities(entities),
                            snap_angle: response.ctx.input(|input| input.modifiers.shift),
                        });
                    } else if let (Some(frame), Some(handle)) = (selected_frame, handle) {
                        let default_preserve_aspect_ratio =
                            self.selection_defaults_to_proportional(entities, editors);
                        let force_preserve_aspect_ratio =
                            self.selection_forces_proportional(entities, editors);
                        let preserve_aspect_ratio = force_preserve_aspect_ratio
                            || default_preserve_aspect_ratio
                                != response.ctx.input(|input| input.modifiers.shift);
                        self.gesture = Some(Gesture::Resize {
                            handle,
                            frame,
                            current: world,
                            originals: self.selected_unlocked_entities(entities),
                            default_preserve_aspect_ratio,
                            force_preserve_aspect_ratio,
                            preserve_aspect_ratio,
                        });
                    } else if let Some(id) = self.entity_at(entities, world) {
                        let entity = entities.iter().find(|entity| entity.id == id).unwrap();
                        if matches!(entity.kind, CanvasEntityKind::Text { .. })
                            && !entity.locked
                            && response.double_clicked()
                        {
                            self.select_entity(entities, id, false);
                            self.editing_text = Some(id);
                            self.focus_text_requested = true;
                            self.gesture = None;
                            return (layer_move, create_block, set_parent, keyboard_action);
                        }
                        if matches!(entity.kind, CanvasEntityKind::DirectEditor { .. }) {
                            let content = direct_editor_layout(entity)
                                .is_some_and(|layout| layout.content.contains(world));
                            if content {
                                self.focused_editor = Some(id);
                                self.gesture = None;
                            } else if response.ctx.input(|input| input.modifiers.shift) {
                                self.select_entity(entities, id, true);
                                self.gesture = None;
                            } else {
                                self.select_entity(entities, id, false);
                                let duplicate = response.ctx.input(|input| input.modifiers.alt);
                                self.begin_move_gesture(entities, world, duplicate);
                            }
                            return (layer_move, create_block, set_parent, keyboard_action);
                        }
                        let additive = response.ctx.input(|input| input.modifiers.shift);
                        if additive {
                            self.select_entity(entities, id, true);
                            self.gesture = None;
                        } else {
                            self.select_entity(entities, id, false);
                            let duplicate = response.ctx.input(|input| input.modifiers.alt);
                            self.begin_move_gesture(entities, world, duplicate);
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
                Tool::Line | Tool::Rectangle | Tool::Text => {
                    self.gesture = Some(Gesture::Create {
                        tool: self.tool,
                        start: world,
                        current: world,
                    });
                }
                Tool::Pen => {
                    self.gesture = Some(Gesture::Pen {
                        points: vec![world],
                    });
                }
                Tool::Block => {
                    if let Some(block) = self.armed_block.take() {
                        self.add_direct_editor(block.id, world);
                        if std::mem::take(&mut self.armed_block_needs_parent) {
                            set_parent = Some(block.id);
                        }
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
            let pen_point_distance = 1.0 / self.render_scale;
            match self.gesture.as_mut() {
                Some(Gesture::Create {
                    tool,
                    start,
                    current,
                }) => {
                    *current = if *tool == Tool::Line
                        && response.ctx.input(|input| input.modifiers.shift)
                    {
                        constrain_point_angle(*start, world, std::f32::consts::FRAC_PI_4)
                    } else {
                        world
                    };
                }
                Some(Gesture::SelectBox { current, .. }) | Some(Gesture::Move { current, .. }) => {
                    *current = world
                }
                Some(Gesture::Rotate {
                    current,
                    snap_angle,
                    ..
                }) => {
                    *current = world;
                    *snap_angle = response.ctx.input(|input| input.modifiers.shift);
                }
                Some(Gesture::Resize {
                    current,
                    default_preserve_aspect_ratio,
                    force_preserve_aspect_ratio,
                    preserve_aspect_ratio,
                    ..
                }) => {
                    *current = world;
                    *preserve_aspect_ratio = *force_preserve_aspect_ratio
                        || *default_preserve_aspect_ratio
                            != response.ctx.input(|input| input.modifiers.shift);
                }
                Some(Gesture::Pen { points }) => {
                    if points
                        .last()
                        .is_none_or(|last| distance(*last, world) > pen_point_distance)
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
        (layer_move, create_block, set_parent, keyboard_action)
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
                            style: CanvasEntityStyle::default(),
                            group_id: None,
                            locked: false,
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
                            style: CanvasEntityStyle::default(),
                            group_id: None,
                            locked: false,
                        })
                    }
                    Tool::Text => {
                        let bounds = WorldRect::from_points(start, current);
                        let dragged = distance(start, current) >= MIN_SIZE;
                        let size = if dragged {
                            CanvasPoint::new(bounds.size().x.max(60.0), bounds.size().y.max(32.0))
                        } else {
                            CanvasPoint::new(180.0, 36.0)
                        };
                        Some(CanvasEntity {
                            id: Uuid::new_v4(),
                            transform: CanvasTransform::new(
                                if dragged { bounds.center() } else { start },
                                size,
                                0.0,
                            ),
                            kind: CanvasEntityKind::Text {
                                text: String::new(),
                                text_style: CanvasTextStyle {
                                    wrap: dragged,
                                    ..CanvasTextStyle::default()
                                },
                            },
                            style: CanvasEntityStyle::default(),
                            group_id: None,
                            locked: false,
                        })
                    }
                    _ => None,
                };
                if let Some(entity) = entity {
                    let text = matches!(entity.kind, CanvasEntityKind::Text { .. });
                    self.add_entity(entity);
                    if text {
                        self.tool = Tool::Select;
                    }
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
                let hits = entities
                    .iter()
                    .filter(|entity| entity_bounds(entity).intersects(selection))
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                for id in hits {
                    self.selection
                        .extend(Self::entity_selection_ids(entities, id));
                }
            }
            Gesture::Move {
                duplicate: true, ..
            } => {
                let additions = preview_entities(&gesture);
                self.block.finish_history_group();
                for entity in additions {
                    self.block.operate(InfiniteCanvasOperation::Add { entity });
                }
            }
            Gesture::Move {
                ref originals,
                duplicate: false,
                ..
            }
            | Gesture::Resize { ref originals, .. }
            | Gesture::Rotate { ref originals, .. } => {
                let updates = preview_entities(&gesture);
                if !updates.is_empty() {
                    self.record_update(originals.clone(), updates, false);
                }
            }
        }
    }

    fn paint(
        &mut self,
        painter: &egui::Painter,
        rect: Rect,
        entities: &[CanvasEntity],
        dependency_details: &HashMap<Uuid, (String, Uuid)>,
        editors: &mut EditorAccess<'_>,
    ) {
        let preview = self
            .gesture
            .as_ref()
            .map(preview_entities)
            .unwrap_or_default();
        let preview: HashMap<_, _> = preview
            .into_iter()
            .map(|entity| (entity.id, entity))
            .collect();
        if entities.is_empty() && self.gesture.is_none() {
            painter.text(
                painter.clip_rect().center(),
                egui::Align2::CENTER_CENTER,
                "Drag to draw  ·  Space-drag to pan  ·  Scroll to move\nDrop or paste images, or use Block to add content",
                egui::FontId::proportional(16.0),
                painter.ctx().global_style().visuals.weak_text_color(),
            );
        }
        for stored in entities {
            let entity = preview.get(&stored.id).unwrap_or(stored);
            if self.editing_text == Some(entity.id) {
                continue;
            }
            paint_entity(
                self,
                painter,
                rect,
                entity,
                dependency_details,
                editors,
                1.0,
            );
        }
        for entity in preview
            .values()
            .filter(|preview| !entities.iter().any(|stored| stored.id == preview.id))
        {
            paint_entity(
                self,
                painter,
                rect,
                entity,
                dependency_details,
                editors,
                1.0,
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
                Tool::Rectangle | Tool::Text => {
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

        if let Some(frame) = self.selected_frame_with_preview(entities, &preview) {
            paint_selection(
                self,
                painter,
                rect,
                frame,
                self.selection_has_unlocked(entities),
                self.selection_has_unlocked(entities)
                    && self.selection_allows_rotation(entities, editors),
            );
        }
    }

    fn selected_frame_with_preview(
        &self,
        entities: &[CanvasEntity],
        preview: &HashMap<Uuid, CanvasEntity>,
    ) -> Option<SelectionFrame> {
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .map(|entity| preview.get(&entity.id).unwrap_or(entity))
            .collect::<Vec<_>>();
        match selected.as_slice() {
            [] => None,
            [entity] => Some(SelectionFrame {
                center: entity.transform.center,
                size: entity.transform.size,
                rotation: entity.transform.rotation,
            }),
            _ => selected
                .into_iter()
                .map(entity_bounds)
                .reduce(WorldRect::union)
                .map(SelectionFrame::from_world_rect),
        }
    }

    fn selected_block_action(
        &mut self,
        context: &egui::Context,
        rect: Rect,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
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
        let cached = editors.client().cached_block(block_id);
        let label = cached.as_ref().map_or_else(
            || "Loading…".to_owned(),
            |block| editors.registry().icon_label(block.block_type, &block.name),
        );
        let mut action = None;
        egui::Area::new(egui::Id::new(("open-canvas-block", entity.id)))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(position + Vec2::new(0.0, 6.0))
            .show(context, |ui| {
                if ui
                    .add_enabled(cached.is_some(), egui::Button::new(label))
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
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let Some(canvas) = self.block.read() else {
            return false;
        };
        let entities = canvas.entities().to_vec();
        drop(canvas);

        let intrinsic = canvas_intrinsic_size(&entities);
        let width = context.corners[0].distance(context.corners[1]);
        let height = context.corners[0].distance(context.corners[3]);
        self.render_scale = (width / intrinsic.x)
            .min(height / intrinsic.y)
            .max(f32::EPSILON);
        let center = Pos2::ZERO
            + context
                .corners
                .iter()
                .fold(Vec2::ZERO, |center, corner| center + corner.to_vec2())
                / context.corners.len() as f32;
        let rect = Rect::from_center_size(center, Vec2::new(width, height));
        let dependencies = self.dependencies.read();
        Self::ensure_dependency_editors(&entities, &dependencies, editors);
        let dependency_details = dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.id,
                    (dependency.name.clone(), dependency.block_type),
                )
            })
            .collect();
        for entity in &entities {
            paint_entity(
                self,
                context.painter,
                rect,
                entity,
                &dependency_details,
                editors,
                context.opacity,
            );
        }
        true
    }

    fn block_created(&mut self, id: Uuid, block_type: Uuid, author: Uuid, name: String) -> bool {
        if let Some(center) = self.pending_block_center.take() {
            self.add_direct_editor(id, center);
            self.tool = Tool::Select;
            true
        } else {
            self.armed_block = Some(CachedBlock {
                id,
                block_type,
                author,
                name,
            });
            self.armed_block_needs_parent = true;
            self.tool = Tool::Block;
            false
        }
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: true,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let canvas = self.block.read()?;
        Some(canvas_intrinsic_size(canvas.entities()))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let canvas = self.block.read()?;
        let entities = canvas.entities().to_vec();
        drop(canvas);
        let focused = focused_direct_editor(self.focused_editor, &entities);
        if self.focused_editor.is_some() && focused.is_none() {
            self.focused_editor = None;
        }

        let mut action = None;
        let mut create_block = None;
        if let Some((_, block_id, _)) = focused {
            ui.horizontal(|ui| {
                if ui
                    .button(format!("{} Back", ICON_ARROW_BACK.codepoint))
                    .clicked()
                {
                    self.focused_editor = None;
                }
            });
            if self.focused_editor.is_some() {
                action = editors.direct_editor_top_bar(block_id, ui, viewport);
            }
        } else {
            create_block = self.show_toolbar(ui, &entities, editors, viewport);
        }
        if let Some(error) = self.image_import_error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.image_import_error = None;
                }
            });
        }
        action.or_else(|| {
            create_block.map(|block_type| EditorAction::CreateBlock {
                block_type,
                parent: Some(self.block.id()),
            })
        })
    }

    fn direct_editor_has_left_sidebar(&self, editors: &mut EditorAccess<'_>) -> bool {
        let Some(canvas) = self.block.read() else {
            return false;
        };
        focused_direct_editor(self.focused_editor, canvas.entities())
            .is_some_and(|(_, block_id, _)| editors.direct_editor_has_left_sidebar(block_id))
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let canvas = self.block.read()?;
        let focused = focused_direct_editor(self.focused_editor, canvas.entities());
        drop(canvas);
        focused.and_then(|(_, block_id, _)| editors.direct_editor_left_sidebar(block_id, ui))
    }

    fn direct_editor_has_right_sidebar(&self, editors: &mut EditorAccess<'_>) -> bool {
        let Some(canvas) = self.block.read() else {
            return false;
        };
        focused_direct_editor(self.focused_editor, canvas.entities()).map_or(true, |focused| {
            editors.direct_editor_has_right_sidebar(focused.1)
        })
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let canvas = self.block.read()?;
        let entities = canvas.entities().to_vec();
        drop(canvas);
        if let Some((_, block_id, _)) = focused_direct_editor(self.focused_editor, &entities) {
            return editors.direct_editor_right_sidebar(block_id, ui);
        }
        if let Some(movement) = self.show_inspector(ui, &entities, editors, true) {
            self.record_action(InfiniteCanvasOperation::Reorder {
                ids: entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
                    .map(|entity| entity.id)
                    .collect(),
                movement,
            });
        }
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.render_scale = scale.max(f32::EPSILON);
        let Some(canvas) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let mut entities = canvas.entities().to_vec();
        drop(canvas);
        let dependencies = self.dependencies.read();
        Self::ensure_dependency_editors(&entities, &dependencies, editors);
        self.autosize_direct_editors(&entities, editors);
        if let Some(current) = self.block.read() {
            entities = current.entities().to_vec();
        }
        let dependency_details = dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.id,
                    (dependency.name.clone(), dependency.block_type),
                )
            })
            .collect();

        let focused = focused_direct_editor(self.focused_editor, &entities);
        if self.focused_editor.is_some() && focused.is_none() {
            self.focused_editor = None;
        }

        let mut action = None;
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let canvas_rect = response.rect;
        let canvas_clip_rect = ui.clip_rect();
        self.viewport_center = self.screen_to_world(canvas_clip_rect.center(), canvas_rect);
        if std::mem::take(&mut self.fit_selection_requested) {
            if let Some(bounds) = self.selected_bounds(&entities) {
                let selection = screen_rect(self, bounds, canvas_rect);
                let available = (canvas_clip_rect.size() - Vec2::splat(40.0)).max(Vec2::splat(1.0));
                let factor = (available.x / selection.width().max(1.0))
                    .min(available.y / selection.height().max(1.0));
                viewport.change_zoom(factor, Some(selection.center()));
                viewport.pan(canvas_clip_rect.center() - selection.center());
            }
        }
        if self.focused_editor.is_none() {
            self.import_dropped_images(&response, editors);
            self.import_clipboard_image(&response, editors);
        }
        self.paint(
            &painter,
            canvas_rect,
            &entities,
            &dependency_details,
            editors,
        );
        self.show_inline_text_editor(ui, canvas_rect, &entities);
        let mut direct_editor_rects = Vec::new();
        if let Some((entity_id, block_id, scale)) =
            focused.filter(|_| self.focused_editor.is_some())
        {
            if let Some(entity) = entities.iter().find(|entity| entity.id == entity_id) {
                let screen = direct_editor_layout(entity)
                    .map(|layout| screen_rect(self, layout.content, canvas_rect))
                    .unwrap_or_else(|| screen_rect(self, entity_bounds(entity), canvas_rect));
                let visible_screen = screen.intersect(canvas_clip_rect);
                direct_editor_rects.push(visible_screen);
                let embedded = ui
                    .new_child(
                        egui::UiBuilder::new()
                            .id_salt(("canvas-direct-editor", entity_id))
                            .max_rect(screen)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    )
                    .scope(|ui| {
                        ui.set_clip_rect(visible_screen.intersect(ui.clip_rect()));
                        ui.set_max_size(screen.size());
                        ui.set_min_size(screen.size());
                        editors.direct_editor_ui(block_id, ui, scale * self.render_scale, viewport)
                    })
                    .inner;
                action = action.or(embedded);
            }
        }

        let (context_layer_move, context_create_block, set_parent, keyboard_action) = self
            .handle_canvas_input(
                &response,
                &entities,
                editors,
                &direct_editor_rects,
                viewport,
            );
        if let Some(movement) = context_layer_move {
            let operation = InfiniteCanvasOperation::Reorder {
                ids: entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
                    .map(|entity| entity.id)
                    .collect(),
                movement,
            };
            self.record_action(operation);
        }
        self.show_picker(ui.ctx(), editors.client());
        if self.focused_editor.is_none() {
            action = action.or_else(|| {
                self.selected_block_action(ui.ctx(), response.rect, &entities, editors)
            });
        }
        if self.gesture.is_some() {
            ui.ctx().request_repaint();
        }
        action
            .or(keyboard_action)
            .or_else(|| {
                set_parent.map(|id| EditorAction::SetParent {
                    id,
                    parent: self.block.id(),
                })
            })
            .or_else(|| {
                context_create_block.map(|block_type| EditorAction::CreateBlock {
                    block_type,
                    parent: Some(self.block.id()),
                })
            })
    }
}

fn duplicate_entities(entities: Vec<CanvasEntity>, offset: CanvasPoint) -> Vec<CanvasEntity> {
    let mut duplicate_groups = HashMap::new();
    entities
        .into_iter()
        .map(|mut entity| {
            entity.id = Uuid::new_v4();
            if let Some(group_id) = entity.group_id {
                entity.group_id = Some(
                    *duplicate_groups
                        .entry(group_id)
                        .or_insert_with(Uuid::new_v4),
                );
            }
            entity.transform.center.x += offset.x;
            entity.transform.center.y += offset.y;
            entity
        })
        .collect()
}

fn focused_direct_editor(
    focused: Option<Uuid>,
    entities: &[CanvasEntity],
) -> Option<(Uuid, Uuid, f32)> {
    let focused = focused?;
    entities.iter().find_map(|entity| match entity.kind {
        CanvasEntityKind::DirectEditor {
            block_id, scale, ..
        } if entity.id == focused => Some((entity.id, block_id, scale)),
        _ => None,
    })
}

fn canvas_intrinsic_size(entities: &[CanvasEntity]) -> Vec2 {
    let bounds = entities.iter().map(entity_bounds).reduce(WorldRect::union);
    bounds
        .map_or_else(
            || Vec2::splat(100.0),
            |bounds| {
                Vec2::new(
                    bounds.min.x.abs().max(bounds.max.x.abs()) * 2.0,
                    bounds.min.y.abs().max(bounds.max.y.abs()) * 2.0,
                )
            },
        )
        .max(Vec2::splat(100.0))
}

fn common_value<T: Copy + PartialEq>(values: impl IntoIterator<Item = T>) -> CommonValue<T> {
    let mut values = values.into_iter();
    let Some(first) = values.next() else {
        return CommonValue::None;
    };
    if values.all(|value| value == first) {
        CommonValue::Uniform(first)
    } else {
        CommonValue::Mixed
    }
}

fn mixed_checkbox(ui: &mut egui::Ui, label: &str, value: CommonValue<bool>) -> Option<bool> {
    let mixed = matches!(value, CommonValue::Mixed);
    let mut checked = matches!(value, CommonValue::Uniform(true));
    let label = if mixed {
        format!("{label} (Mixed)")
    } else {
        label.into()
    };
    ui.checkbox(&mut checked, label)
        .changed()
        .then_some(checked)
}

const COLOR_PRESETS: [(&str, CanvasColor); 5] = [
    ("Default", CanvasColor::Auto),
    (
        "Red",
        CanvasColor::Rgba {
            red: 224,
            green: 49,
            blue: 49,
            alpha: 255,
        },
    ),
    (
        "Orange",
        CanvasColor::Rgba {
            red: 240,
            green: 140,
            blue: 0,
            alpha: 255,
        },
    ),
    (
        "Green",
        CanvasColor::Rgba {
            red: 47,
            green: 158,
            blue: 68,
            alpha: 255,
        },
    ),
    (
        "Blue",
        CanvasColor::Rgba {
            red: 25,
            green: 113,
            blue: 194,
            alpha: 255,
        },
    ),
];

fn color_menu(
    ui: &mut egui::Ui,
    label: &str,
    value: CommonValue<CanvasColor>,
) -> Option<CanvasColor> {
    let mut changed = None;
    let current = match value {
        CommonValue::Uniform(color) => color,
        CommonValue::Mixed | CommonValue::None => CanvasColor::Auto,
    };
    ui.horizontal(|ui| {
        ui.label(label);
        for (name, color) in COLOR_PRESETS {
            if color_button(ui, name, color, value == CommonValue::Uniform(color)).clicked() {
                changed = Some(color);
            }
        }
        let mut color = resolve_color(current, ui.visuals().text_color()).to_srgba_unmultiplied();
        if ui
            .color_edit_button_srgba_unmultiplied(&mut color)
            .on_hover_text("Custom color")
            .changed()
        {
            changed = Some(CanvasColor::Rgba {
                red: color[0],
                green: color[1],
                blue: color[2],
                alpha: color[3],
            });
        }
    });
    changed
}

fn fill_color_menu(
    ui: &mut egui::Ui,
    value: CommonValue<Option<CanvasColor>>,
) -> Option<Option<CanvasColor>> {
    let mut changed = None;
    let current = match value {
        CommonValue::Uniform(Some(color)) => color,
        CommonValue::Uniform(None) | CommonValue::Mixed | CommonValue::None => CanvasColor::Auto,
    };
    ui.horizontal(|ui| {
        ui.label("Fill");
        if ui
            .selectable_label(value == CommonValue::Uniform(None), ICON_FORMAT_COLOR_RESET)
            .on_hover_text("No fill")
            .clicked()
        {
            changed = Some(None);
        }
        for (name, color) in COLOR_PRESETS {
            if color_button(ui, name, color, value == CommonValue::Uniform(Some(color))).clicked() {
                changed = Some(Some(color));
            }
        }
        let mut color = resolve_color(current, ui.visuals().text_color()).to_srgba_unmultiplied();
        if ui
            .color_edit_button_srgba_unmultiplied(&mut color)
            .on_hover_text("Custom color")
            .changed()
        {
            changed = Some(Some(CanvasColor::Rgba {
                red: color[0],
                green: color[1],
                blue: color[2],
                alpha: color[3],
            }));
        }
    });
    changed
}

fn color_button(
    ui: &mut egui::Ui,
    name: &str,
    color: CanvasColor,
    selected: bool,
) -> egui::Response {
    let color = resolve_color(color, ui.visuals().text_color());
    ui.selectable_label(selected, ICON_CIRCLE.rich_text().color(color))
        .on_hover_text(name)
}

fn resolve_color(color: CanvasColor, auto: Color32) -> Color32 {
    match color {
        CanvasColor::Auto => auto,
        CanvasColor::Rgba {
            red,
            green,
            blue,
            alpha,
        } => Color32::from_rgba_unmultiplied(red, green, blue, alpha),
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

#[derive(Clone, Copy)]
struct DirectEditorLayout {
    title_bar: WorldRect,
    content: WorldRect,
}

fn direct_editor_entity_size(intrinsic: Vec2, scale: f32) -> CanvasPoint {
    CanvasPoint::new(
        ((intrinsic.x + DIRECT_EDITOR_PADDING * 2.0) * scale).max(MIN_SIZE),
        ((intrinsic.y
            + DIRECT_EDITOR_PADDING * 2.0
            + DIRECT_EDITOR_TITLE_HEIGHT
            + DIRECT_EDITOR_TITLE_GAP)
            * scale)
            .max(MIN_SIZE),
    )
}

fn direct_editor_to_preview(entity: &CanvasEntity, block_id: Uuid) -> Option<CanvasEntity> {
    let content = direct_editor_layout(entity)?.content;
    let content_size = content.size();
    let mut preview = entity.clone();
    preview.kind = CanvasEntityKind::Block { block_id };
    preview.transform.center = content.center();
    preview.transform.size =
        CanvasPoint::new(content_size.x.max(MIN_SIZE), content_size.y.max(MIN_SIZE));
    preview.transform.rotation = 0.0;
    Some(preview)
}

fn preview_to_direct_editor(
    entity: &CanvasEntity,
    block_id: Uuid,
    intrinsic: Vec2,
) -> CanvasEntity {
    let content = entity_bounds(entity);
    let content_size = content.size();
    let scale = (content_size.x / intrinsic.x)
        .min(content_size.y / intrinsic.y)
        .max(f32::EPSILON);
    let mut direct = entity.clone();
    direct.kind = CanvasEntityKind::DirectEditor { block_id, scale };
    direct.transform.center = CanvasPoint::new(
        content.center().x,
        content.center().y - (DIRECT_EDITOR_TITLE_HEIGHT + DIRECT_EDITOR_TITLE_GAP) * scale * 0.5,
    );
    direct.transform.size =
        direct_editor_entity_size(Vec2::new(content_size.x, content_size.y), scale);
    direct.transform.rotation = 0.0;
    direct
}

fn direct_editor_layout(entity: &CanvasEntity) -> Option<DirectEditorLayout> {
    let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind else {
        return None;
    };
    let bounds = entity_bounds(entity);
    let padding = DIRECT_EDITOR_PADDING * scale;
    let title_height = DIRECT_EDITOR_TITLE_HEIGHT * scale;
    let title_gap = DIRECT_EDITOR_TITLE_GAP * scale;
    Some(DirectEditorLayout {
        title_bar: WorldRect {
            min: CanvasPoint::new(bounds.min.x + padding, bounds.min.y + padding),
            max: CanvasPoint::new(
                bounds.max.x - padding,
                bounds.min.y + padding + title_height,
            ),
        },
        content: WorldRect {
            min: CanvasPoint::new(
                bounds.min.x + padding,
                bounds.min.y + padding + title_height + title_gap,
            ),
            max: CanvasPoint::new(bounds.max.x - padding, bounds.max.y - padding),
        },
    })
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
        CanvasEntityKind::Rectangle if entity.style.fill.is_none() => {
            let local = world_to_local(entity.transform, point);
            let hit_radius = radius + entity.style.line_width * 0.5;
            let x_radius = hit_radius / entity.transform.size.x.max(MIN_SIZE);
            let y_radius = hit_radius / entity.transform.size.y.max(MIN_SIZE);
            let near_vertical_edge =
                (local.x.abs() - 0.5).abs() <= x_radius && local.y.abs() <= 0.5 + y_radius;
            let near_horizontal_edge =
                (local.y.abs() - 0.5).abs() <= y_radius && local.x.abs() <= 0.5 + x_radius;
            near_vertical_edge || near_horizontal_edge
        }
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
    frame: SelectionFrame,
    rect: Rect,
    world: CanvasPoint,
) -> Option<ResizeHandle> {
    let pointer = editor.world_to_screen(world, rect);
    resize_handles(frame)
        .into_iter()
        .find_map(|(handle, point)| {
            (editor.world_to_screen(point, rect).distance(pointer) <= HANDLE_RADIUS + 3.0)
                .then_some(handle)
        })
}

fn resize_handles(frame: SelectionFrame) -> [(ResizeHandle, CanvasPoint); 8] {
    [
        (
            ResizeHandle { x: -1, y: -1 },
            frame.point(CanvasPoint::new(-0.5, -0.5)),
        ),
        (
            ResizeHandle { x: 0, y: -1 },
            frame.point(CanvasPoint::new(0.0, -0.5)),
        ),
        (
            ResizeHandle { x: 1, y: -1 },
            frame.point(CanvasPoint::new(0.5, -0.5)),
        ),
        (
            ResizeHandle { x: 1, y: 0 },
            frame.point(CanvasPoint::new(0.5, 0.0)),
        ),
        (
            ResizeHandle { x: 1, y: 1 },
            frame.point(CanvasPoint::new(0.5, 0.5)),
        ),
        (
            ResizeHandle { x: 0, y: 1 },
            frame.point(CanvasPoint::new(0.0, 0.5)),
        ),
        (
            ResizeHandle { x: -1, y: 1 },
            frame.point(CanvasPoint::new(-0.5, 0.5)),
        ),
        (
            ResizeHandle { x: -1, y: 0 },
            frame.point(CanvasPoint::new(-0.5, 0.0)),
        ),
    ]
}

fn rotate_handle_at(editor: &InfiniteCanvasEditor, frame: SelectionFrame, rect: Rect) -> Pos2 {
    let top = editor.world_to_screen(frame.point(CanvasPoint::new(0.0, -0.5)), rect);
    top + Vec2::angled(frame.rotation - std::f32::consts::FRAC_PI_2) * ROTATE_OFFSET
}

fn preview_entities(gesture: &Gesture) -> Vec<CanvasEntity> {
    match gesture {
        Gesture::Move {
            start,
            current,
            originals,
            ..
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
            frame,
            current,
            originals,
            preserve_aspect_ratio,
            ..
        } => resize_entities(*handle, *frame, *current, originals, *preserve_aspect_ratio),
        Gesture::Rotate {
            frame,
            start_angle,
            current,
            originals,
            snap_angle,
        } => {
            let center = frame.center;
            let current_angle = (current.y - center.y).atan2(current.x - center.x);
            let mut delta = current_angle - start_angle;
            if *snap_angle {
                let step = 15.0_f32.to_radians();
                delta = (delta / step).round() * step;
            }
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

fn constrain_point_angle(start: CanvasPoint, current: CanvasPoint, step: f32) -> CanvasPoint {
    let delta = CanvasPoint::new(current.x - start.x, current.y - start.y);
    let length = delta.x.hypot(delta.y);
    let angle = (delta.y.atan2(delta.x) / step).round() * step;
    CanvasPoint::new(
        start.x + length * angle.cos(),
        start.y + length * angle.sin(),
    )
}

fn resize_entities(
    handle: ResizeHandle,
    frame: SelectionFrame,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
) -> Vec<CanvasEntity> {
    let current = rotate(
        CanvasPoint::new(current.x - frame.center.x, current.y - frame.center.y),
        -frame.rotation,
    );
    let local_originals = originals
        .iter()
        .cloned()
        .map(|mut entity| {
            entity.transform.center = rotate(
                CanvasPoint::new(
                    entity.transform.center.x - frame.center.x,
                    entity.transform.center.y - frame.center.y,
                ),
                -frame.rotation,
            );
            entity.transform.rotation -= frame.rotation;
            entity
        })
        .collect::<Vec<_>>();
    resize_entities_axis(
        handle,
        frame.local_bounds(),
        current,
        &local_originals,
        preserve_aspect_ratio,
    )
    .into_iter()
    .map(|mut entity| {
        let center = rotate(entity.transform.center, frame.rotation);
        entity.transform.center =
            CanvasPoint::new(frame.center.x + center.x, frame.center.y + center.y);
        entity.transform.rotation += frame.rotation;
        entity
    })
    .collect()
}

fn resize_entities_axis(
    handle: ResizeHandle,
    bounds: WorldRect,
    current: CanvasPoint,
    originals: &[CanvasEntity],
    preserve_aspect_ratio: bool,
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
    if preserve_aspect_ratio {
        resized = proportional_resize_bounds(handle, bounds, resized);
    }
    let resized_size = resized.size();
    let scale_x = if handle.x == 0 && !preserve_aspect_ratio {
        1.0
    } else {
        resized_size.x / original_size.x.max(MIN_SIZE)
    };
    let scale_y = if handle.y == 0 && !preserve_aspect_ratio {
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
                        if handle.x == 0 && !preserve_aspect_ratio {
                            point.x
                        } else {
                            resized.min.x
                                + (point.x - bounds.min.x) / original_size.x.max(MIN_SIZE)
                                    * resized_size.x
                        },
                        if handle.y == 0 && !preserve_aspect_ratio {
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
                if handle.x == 0 && !preserve_aspect_ratio {
                    entity.transform.center.x
                } else {
                    resized.min.x + unit_x * resized_size.x
                },
                if handle.y == 0 && !preserve_aspect_ratio {
                    entity.transform.center.y
                } else {
                    resized.min.y + unit_y * resized_size.y
                },
            );
            entity.transform.size.x = (entity.transform.size.x * scale_x).max(MIN_SIZE);
            entity.transform.size.y = (entity.transform.size.y * scale_y).max(MIN_SIZE);
            if let CanvasEntityKind::DirectEditor { scale, .. } = &mut entity.kind {
                *scale = (*scale * scale_x).max(f32::EPSILON);
            }
            entity
        })
        .collect()
}

fn proportional_resize_bounds(
    handle: ResizeHandle,
    bounds: WorldRect,
    resized: WorldRect,
) -> WorldRect {
    let original_width = bounds.size().x.max(MIN_SIZE);
    let original_height = bounds.size().y.max(MIN_SIZE);
    let resized_width = resized.size().x.max(MIN_SIZE);
    let resized_height = resized.size().y.max(MIN_SIZE);
    let scale = match (handle.x, handle.y) {
        (0, _) => resized_height / original_height,
        (_, 0) => resized_width / original_width,
        _ => {
            let horizontal = resized_width / original_width;
            let vertical = resized_height / original_height;
            if (horizontal - 1.0).abs() >= (vertical - 1.0).abs() {
                horizontal
            } else {
                vertical
            }
        }
    }
    .max(MIN_SIZE / original_width)
    .max(MIN_SIZE / original_height);
    let width = original_width * scale;
    let height = original_height * scale;
    let center = bounds.center();
    let (min_x, max_x) = match handle.x {
        -1 => (bounds.max.x - width, bounds.max.x),
        1 => (bounds.min.x, bounds.min.x + width),
        _ => (center.x - width * 0.5, center.x + width * 0.5),
    };
    let (min_y, max_y) = match handle.y {
        -1 => (bounds.max.y - height, bounds.max.y),
        1 => (bounds.min.y, bounds.min.y + height),
        _ => (center.y - height * 0.5, center.y + height * 0.5),
    };
    WorldRect {
        min: CanvasPoint::new(min_x, min_y),
        max: CanvasPoint::new(max_x, max_y),
    }
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
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
    }
}

fn paint_entity(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    entity: &CanvasEntity,
    dependency_details: &HashMap<Uuid, (String, Uuid)>,
    editors: &mut EditorAccess<'_>,
    parent_opacity: f32,
) {
    let auto = painter.ctx().global_style().visuals.text_color();
    let opacity = (entity.style.opacity * parent_opacity).clamp(0.0, 1.0);
    let color = with_opacity(resolve_color(entity.style.foreground, auto), opacity);
    let stroke = Stroke::new(
        (entity.style.line_width.max(0.0) * editor.render_scale).max(0.1),
        color,
    );
    match &entity.kind {
        CanvasEntityKind::Line => {
            paint_styled_line(
                painter,
                editor.world_to_screen(
                    local_to_world(entity.transform, CanvasPoint::new(-0.5, 0.0)),
                    rect,
                ),
                editor.world_to_screen(
                    local_to_world(entity.transform, CanvasPoint::new(0.5, 0.0)),
                    rect,
                ),
                stroke,
                entity.style,
                editor.render_scale,
            );
        }
        CanvasEntityKind::Rectangle => {
            let points: Vec<_> = rounded_rectangle_points(entity, entity.style.corner_radius)
                .into_iter()
                .map(|point| editor.world_to_screen(point, rect))
                .collect();
            let fill = entity
                .style
                .fill
                .map(|fill| with_opacity(resolve_color(fill, auto), opacity))
                .unwrap_or(Color32::TRANSPARENT);
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        CanvasEntityKind::Text { text, text_style } => {
            let center = editor.world_to_screen(entity.transform.center, rect);
            let font_size = (text_style.font_size * editor.render_scale).clamp(4.0, 256.0);
            let font = egui::FontId::proportional(font_size);
            let wrap_width = if text_style.wrap {
                (entity.transform.size.x * editor.render_scale).max(1.0)
            } else {
                f32::INFINITY
            };
            let mut job = egui::text::LayoutJob::simple(text.clone(), font, color, wrap_width);
            job.halign = match text_style.alignment {
                CanvasTextAlign::Left => egui::Align::LEFT,
                CanvasTextAlign::Center => egui::Align::Center,
                CanvasTextAlign::Right => egui::Align::RIGHT,
            };
            job.sections[0].format.line_height = Some(font_size * text_style.line_height.max(0.5));
            let galley = painter.layout_job(job);
            let position = center - galley.size() * 0.5;
            if text_style.weight == CanvasTextWeight::Bold {
                painter.add(
                    egui::epaint::TextShape::new(
                        position + Vec2::new(0.6 * editor.render_scale, 0.0),
                        galley.clone(),
                        color,
                    )
                    .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
                );
            }
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
        CanvasEntityKind::Block { block_id } => {
            let corners = entity_corners(entity).map(|point| editor.world_to_screen(point, rect));
            if editors.render(
                *block_id,
                BlockRenderContext {
                    painter,
                    corners,
                    opacity,
                },
            ) {
                return;
            }
            painter.add(egui::Shape::convex_polygon(
                corners.to_vec(),
                with_opacity(Color32::from_gray(35), opacity),
                Stroke::NONE,
            ));
            let center = editor.world_to_screen(entity.transform.center, rect);
            let title = dependency_details
                .get(block_id)
                .map(|(title, _)| title.clone())
                .unwrap_or_else(|| "Loading…".into());
            let title_galley = painter.layout_no_wrap(
                title,
                egui::FontId::proportional((18.0 * editor.render_scale).clamp(8.0, 42.0)),
                color,
            );
            let preview_galley = painter.layout_no_wrap(
                "(TODO: preview)".into(),
                egui::FontId::proportional((12.0 * editor.render_scale).clamp(7.0, 30.0)),
                color,
            );
            let gap = (4.0 * editor.render_scale).clamp(2.0, 10.0);
            let total_height = title_galley.size().y + gap + preview_galley.size().y;
            let title_center_offset = -total_height * 0.5 + title_galley.size().y * 0.5;
            let preview_center_offset = total_height * 0.5 - preview_galley.size().y * 0.5;
            let (sin, cos) = entity.transform.rotation.sin_cos();
            let rotated_offset = |offset: f32| Vec2::new(-offset * sin, offset * cos);
            let title_center = center + rotated_offset(title_center_offset);
            let preview_center = center + rotated_offset(preview_center_offset);
            painter.add(
                egui::epaint::TextShape::new(
                    title_center - title_galley.size() * 0.5,
                    title_galley,
                    color,
                )
                .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
            painter.add(
                egui::epaint::TextShape::new(
                    preview_center - preview_galley.size() * 0.5,
                    preview_galley,
                    color,
                )
                .with_angle_and_anchor(entity.transform.rotation, egui::Align2::CENTER_CENTER),
            );
        }
        CanvasEntityKind::DirectEditor { block_id, scale } => {
            let Some(layout) = direct_editor_layout(entity) else {
                return;
            };
            let outer = screen_rect(editor, entity_bounds(entity), rect);
            let title_bar = screen_rect(editor, layout.title_bar, rect);
            let content = screen_rect(editor, layout.content, rect);
            let visuals = &painter.ctx().global_style().visuals;
            painter.rect(
                outer,
                (6.0 * scale * editor.render_scale).clamp(2.0, 12.0),
                with_opacity(visuals.panel_fill, opacity),
                Stroke::new(
                    editor.render_scale.max(0.5),
                    with_opacity(visuals.widgets.noninteractive.bg_stroke.color, opacity),
                ),
                egui::StrokeKind::Inside,
            );
            painter.rect_filled(
                title_bar,
                (4.0 * scale * editor.render_scale).clamp(1.0, 8.0),
                with_opacity(visuals.widgets.inactive.bg_fill, opacity),
            );

            let content_corners = [
                content.left_top(),
                content.right_top(),
                content.right_bottom(),
                content.left_bottom(),
            ];
            if !editors.render(
                *block_id,
                BlockRenderContext {
                    painter,
                    corners: content_corners,
                    opacity,
                },
            ) {
                painter.rect_filled(content, 0.0, with_opacity(Color32::from_gray(35), opacity));
            }

            let (title, block_type) = dependency_details
                .get(block_id)
                .map(|(title, block_type)| (title.as_str(), Some(*block_type)))
                .unwrap_or(("Loading...", None));
            let font_size = (16.0 * scale * editor.render_scale).clamp(8.0, 32.0);
            let left_padding = (6.0 * scale * editor.render_scale).clamp(3.0, 12.0);
            let title_painter = painter.with_clip_rect(title_bar);
            let mut title_x = title_bar.left() + left_padding;
            if let Some(icon) =
                block_type.and_then(|block_type| editors.registry().icon(block_type))
            {
                title_painter.text(
                    Pos2::new(title_x, title_bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon.codepoint,
                    egui::FontId::new(font_size, icon.font_family()),
                    color,
                );
                title_x += font_size + left_padding;
            }
            title_painter.text(
                Pos2::new(title_x, title_bar.center().y),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::proportional(font_size),
                color,
            );
        }
    }
}

fn paint_styled_line(
    painter: &egui::Painter,
    start: Pos2,
    end: Pos2,
    stroke: Stroke,
    style: CanvasEntityStyle,
    zoom: f32,
) {
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        return;
    }
    let direction = delta / length;
    let arrow_size = ((style.line_width * 4.0).max(8.0) * zoom).min(length * 0.4);
    let inset = arrow_size * 0.75;
    let shaft_start = if style.arrow_start {
        start + direction * inset
    } else {
        start
    };
    let shaft_end = if style.arrow_end {
        end - direction * inset
    } else {
        end
    };

    if style.dashed {
        let dash = (stroke.width * 3.0).max(2.0);
        let gap = (stroke.width * 2.0).max(2.0);
        painter.extend(egui::Shape::dashed_line(
            &[shaft_start, shaft_end],
            stroke,
            dash,
            gap,
        ));
    } else {
        painter.line_segment([shaft_start, shaft_end], stroke);
    }

    if style.arrow_start {
        painter.add(arrowhead(start, direction, arrow_size, stroke.color));
    }
    if style.arrow_end {
        painter.add(arrowhead(end, -direction, arrow_size, stroke.color));
    }
}

fn arrowhead(tip: Pos2, inward: Vec2, size: f32, color: Color32) -> egui::Shape {
    let base = tip + inward * size;
    let perpendicular = Vec2::new(-inward.y, inward.x) * size * 0.45;
    egui::Shape::convex_polygon(
        vec![tip, base + perpendicular, base - perpendicular],
        color,
        Stroke::NONE,
    )
}

fn rounded_rectangle_points(entity: &CanvasEntity, radius: f32) -> Vec<CanvasPoint> {
    let width = entity.transform.size.x.abs().max(0.001);
    let height = entity.transform.size.y.abs().max(0.001);
    let radius = radius.max(0.0).min(width * 0.5).min(height * 0.5);
    if radius <= f32::EPSILON {
        return entity_corners(entity).into();
    }

    const STEPS: usize = 6;
    let corners = [
        (width * 0.5 - radius, -height * 0.5 + radius, -90.0_f32),
        (width * 0.5 - radius, height * 0.5 - radius, 0.0),
        (-width * 0.5 + radius, height * 0.5 - radius, 90.0),
        (-width * 0.5 + radius, -height * 0.5 + radius, 180.0),
    ];
    let mut points = Vec::with_capacity(corners.len() * (STEPS + 1));
    for (center_x, center_y, start_degrees) in corners {
        for step in 0..=STEPS {
            let angle = (start_degrees + 90.0 * step as f32 / STEPS as f32).to_radians();
            let local_x = center_x + radius * angle.cos();
            let local_y = center_y + radius * angle.sin();
            points.push(local_to_world(
                entity.transform,
                CanvasPoint::new(local_x / width, local_y / height),
            ));
        }
    }
    points
}

fn with_opacity(color: Color32, opacity: f32) -> Color32 {
    let [red, green, blue, alpha] = color.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(
        red,
        green,
        blue,
        (alpha as f32 * opacity.clamp(0.0, 1.0)).round() as u8,
    )
}

fn paint_selection(
    editor: &InfiniteCanvasEditor,
    painter: &egui::Painter,
    rect: Rect,
    frame: SelectionFrame,
    allow_transform: bool,
    allow_rotation: bool,
) {
    let corners = [
        CanvasPoint::new(-0.5, -0.5),
        CanvasPoint::new(0.5, -0.5),
        CanvasPoint::new(0.5, 0.5),
        CanvasPoint::new(-0.5, 0.5),
    ]
    .map(|point| editor.world_to_screen(frame.point(point), rect));
    painter.add(egui::Shape::closed_line(
        corners.to_vec(),
        Stroke::new(1.0, Color32::LIGHT_BLUE),
    ));
    if allow_transform {
        for (_, point) in resize_handles(frame) {
            painter.circle_filled(
                editor.world_to_screen(point, rect),
                HANDLE_RADIUS,
                Color32::LIGHT_BLUE,
            );
        }
    }
    if allow_rotation {
        let rotate = rotate_handle_at(editor, frame, rect);
        let top = editor.world_to_screen(frame.point(CanvasPoint::new(0.0, -0.5)), rect);
        painter.line_segment([top, rotate], Stroke::new(1.0, Color32::LIGHT_BLUE));
        painter.circle_filled(rotate, HANDLE_RADIUS, Color32::LIGHT_BLUE);
    }
}
