use super::*;

#[derive(Default)]
pub struct CanvasApp {
    editor: Option<InfiniteCanvasEditor>,
    creation: Option<Arc<BlockClient>>,
}

impl CanvasApp {
    fn with<T>(&mut self, run: impl FnOnce(&mut InfiniteCanvasEditor, &Access) -> T) -> Option<T> {
        let editor = self.editor.as_mut()?;
        let access = editor.access.clone();
        Some(run(editor, &access))
    }
}

impl block_editor_plugin::App for CanvasApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        let block = client.get_block(block_id);
        self.editor = Some(InfiniteCanvasEditor::new(block, host, client));
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(InfiniteCanvas::new()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = &mut self.editor else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let access = editor.access.clone();
        editor.canvas_ui(ui, &access);
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let access = editor.access.clone();
        editor.preview_ui(ui, &access);
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.with(|editor, access| editor.top_bar_ui(ui, access));
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.with(|editor, access| editor.right_sidebar_ui(ui, access));
    }

    fn intrinsic_size(&mut self) -> Option<Vec2> {
        let editor = self.editor.as_ref()?;
        let canvas = editor.block.read()?;
        let region = canvas
            .preview_region()
            .unwrap_or_else(|| preview_region_for_entities(canvas.entities()));
        Some(Vec2::new(region.size.x, region.size.y))
    }

    fn set_intrinsic_size(&mut self, size: Vec2) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        let Some(region) = editor
            .block
            .read()
            .and_then(|canvas| canvas.preview_region())
        else {
            return;
        };
        let updated = CanvasPreviewRegion::new(
            region.center,
            CanvasPoint::new(size.x.max(MIN_SIZE), size.y.max(MIN_SIZE)),
        );
        if (updated.size.x - region.size.x).abs() < 0.01
            && (updated.size.y - region.size.y).abs() < 0.01
        {
            return;
        }
        editor.record_action(InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(updated),
        });
    }

    fn presence_visible(&mut self, visible: bool) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        editor.presence_visible = visible;
        if !visible {
            editor.access.host().set_presence::<CanvasCursor>(None);
        }
    }

    fn reveal_presence(&mut self, client_id: u64) {
        if let Some(editor) = &mut self.editor {
            editor.pending_presence_reveal = Some(client_id);
        }
    }

    fn replace_child(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(editor) = &mut self.editor else {
            return false;
        };
        editor.replace_referenced_block(old, new)
    }
}

impl InfiniteCanvasEditor {
    fn publish_cursor_presence(&self) {
        if !self.presence_visible {
            return;
        }
        self.access.host().set_presence(Some(&CanvasCursor {
            pointer: self.pointer_world,
            selection: self.selection.iter().copied().collect(),
        }));
    }

    fn replace_referenced_block(&mut self, old: Uuid, new: Uuid) -> bool {
        let Some(canvas) = self.block.read() else {
            return false;
        };
        let entities = canvas.entities().to_vec();
        drop(canvas);
        let old_reference = BlockRef::Direct(old);
        let new_reference = BlockRef::Direct(new);
        let mut replaced = Vec::new();
        for entity in &entities {
            let updated = match entity.kind {
                CanvasEntityKind::Block { block_id } if block_id == old_reference => {
                    CanvasEntityKind::Block {
                        block_id: new_reference,
                    }
                }
                CanvasEntityKind::DirectEditor { block_id, scale } if block_id == old_reference => {
                    CanvasEntityKind::DirectEditor {
                        block_id: new_reference,
                        scale,
                    }
                }
                _ => continue,
            };
            replaced.push(CanvasEntity {
                kind: updated,
                ..entity.clone()
            });
        }
        if replaced.is_empty() {
            return false;
        }
        self.record_action(InfiniteCanvasOperation::Update { entities: replaced });
        true
    }

    fn top_bar_ui(&mut self, ui: &mut egui::Ui, editors: &Access) {
        let Some(canvas) = self.block.read() else {
            return;
        };
        let entities = canvas.entities().to_vec();
        drop(canvas);
        let mut viewport = DirectEditorViewport::new(editors.host().clone(), view_scale(editors));
        self.show_toolbar(ui, &entities, editors, &mut viewport);
        if let Some(error) = self.image_import_error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.image_import_error = None;
                }
            });
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui, editors: &Access) {
        let Some(canvas) = self.block.read() else {
            return;
        };
        let entities = canvas.entities().to_vec();
        drop(canvas);
        let (movement, action) = self.show_inspector(ui, &entities, editors, true);
        if let Some(movement) = movement {
            self.record_action(InfiniteCanvasOperation::Reorder {
                ids: entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
                    .map(|entity| entity.id)
                    .collect(),
                movement,
            });
        }
        self.take_action(action, editors);
    }

    fn take_action(&self, action: Option<EditorAction>, editors: &Access) {
        if let Some(EditorAction::OpenBlock { id, block_type }) = action {
            editors.host().open_block(id, block_type);
        }
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, editors: &Access) {
        editors.begin_frame();
        self.reference_cache.poll();
        let Some(canvas) = self.block.read() else {
            return;
        };
        let entities = canvas.entities().to_vec();
        let preview_region = canvas
            .preview_region()
            .unwrap_or_else(|| preview_region_for_entities(&entities));
        drop(canvas);

        let available = ui.available_rect_before_wrap();
        let intrinsic = Vec2::new(
            preview_region.size.x.max(MIN_SIZE),
            preview_region.size.y.max(MIN_SIZE),
        );
        self.render_scale = (available.width() / intrinsic.x)
            .min(available.height() / intrinsic.y)
            .max(f32::EPSILON);
        let rect = Rect::from_center_size(
            available.center()
                - Vec2::new(preview_region.center.x, preview_region.center.y) * self.render_scale,
            available.size(),
        );
        let dependencies = self.dependencies.read();
        let dependency_details = dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.id,
                    BlockLabel::for_reference(editors.registry(), dependency),
                )
            })
            .collect();
        let painter = ui.painter().clone();
        for entity in &entities {
            paint_entity(
                self,
                ui,
                &painter,
                rect,
                entity,
                &dependency_details,
                editors,
                false,
                1.0,
            );
        }
    }

    fn canvas_ui(&mut self, ui: &mut egui::Ui, editors: &Access) {
        editors.begin_frame();
        self.publish_cursor_presence();
        self.poll_references();
        let Some(canvas) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let mut entities = canvas.entities().to_vec();
        drop(canvas);
        let dependencies = self.dependencies.read();
        self.autosize_direct_editors(&entities, editors);
        if let Some(current) = self.block.read() {
            entities = current.entities().to_vec();
        }
        let dependency_details = dependencies
            .iter()
            .map(|dependency| {
                (
                    dependency.id,
                    BlockLabel::for_reference(editors.registry(), dependency),
                )
            })
            .collect::<HashMap<_, _>>();

        let focused = focused_direct_editor(self.focused_editor, &entities);
        if self.focused_editor.is_some() && focused.is_none() {
            self.focused_editor = None;
        }

        let mut action = None;
        let (response, painter) =
            ui.allocate_painter(ui.available_size(), egui::Sense::click_and_drag());
        let canvas_rect = editors.host().view().unwrap_or(response.rect);
        let canvas_clip_rect = ui.clip_rect();
        self.render_scale = view_scale(editors);
        let mut viewport = DirectEditorViewport::new(editors.host().clone(), self.render_scale);
        self.viewport_center = self.screen_to_world(canvas_clip_rect.center(), canvas_rect);
        if std::mem::take(&mut self.fit_selection_requested) {
            if let Some(bounds) = self.selected_bounds(&entities) {
                fit_into_view(
                    &mut viewport,
                    canvas_clip_rect,
                    screen_rect(self, bounds, canvas_rect),
                );
            }
        }
        if let Some(id) = self.fit_entity_requested.take() {
            if let Some(bounds) = entities
                .iter()
                .find(|entity| entity.id == id)
                .and_then(|entity| direct_editor_layout(entity).map(|layout| layout.content))
            {
                fit_into_view(
                    &mut viewport,
                    canvas_clip_rect,
                    screen_rect(self, bounds, canvas_rect),
                );
            }
        }
        if std::mem::take(&mut self.fit_preview_region_requested) {
            if let Some(region) = self.block.read().and_then(|canvas| canvas.preview_region()) {
                fit_into_view(
                    &mut viewport,
                    canvas_clip_rect,
                    screen_rect(self, preview_region_bounds(region), canvas_rect),
                );
                viewport.resume_auto_fit();
            }
        }
        if let Some(client_id) = std::mem::take(&mut self.pending_presence_reveal) {
            if let Some((_, cursor)) = editors
                .host()
                .presence::<CanvasCursor>()
                .into_iter()
                .find(|(id, _)| *id == client_id)
            {
                let target = cursor.pointer.or_else(|| {
                    entities
                        .iter()
                        .filter(|entity| cursor.selection.contains(&entity.id))
                        .map(entity_bounds)
                        .reduce(WorldRect::union)
                        .map(|bounds| bounds.center())
                });
                if let Some(target) = target {
                    let screen = self.world_to_screen(target, canvas_rect);
                    viewport.pan(canvas_clip_rect.center() - screen);
                }
            }
        }
        self.import_picked_image(editors);
        if self.focused_editor.is_none() {
            self.import_dropped_images(&response, canvas_rect, editors);
            self.import_clipboard_image(ui, &response, canvas_rect, editors);
        }
        self.paint(
            ui,
            &painter,
            canvas_rect,
            &entities,
            &dependency_details,
            editors,
        );
        let gesture_preview = self
            .gesture
            .as_ref()
            .map(preview_entities)
            .unwrap_or_default();
        let gesture_preview = gesture_preview
            .iter()
            .map(|entity| (entity.id, entity))
            .collect::<HashMap<_, _>>();
        let mut direct_editor_rects = Vec::new();
        let displayed_entities = entities
            .iter()
            .map(|entity| gesture_preview.get(&entity.id).copied().unwrap_or(entity))
            .chain(
                gesture_preview
                    .values()
                    .copied()
                    .filter(|preview| !entities.iter().any(|entity| entity.id == preview.id)),
            )
            .cloned()
            .collect::<Vec<_>>();
        for entity in &displayed_entities {
            let CanvasEntityKind::DirectEditor { block_id, .. } = entity.kind else {
                continue;
            };
            let Some(block_id) = self.resolve_block_id(editors, block_id) else {
                continue;
            };
            let interaction = editors
                .direct_editor_interaction(block_id)
                .unwrap_or(DirectEditorInteraction::Preview);
            let is_focused = self.focused_editor == Some(entity.id);
            if interaction == DirectEditorInteraction::Preview && !is_focused {
                continue;
            }
            let screen = direct_editor_layout(entity)
                .map(|layout| screen_rect(self, layout.content, canvas_rect))
                .unwrap_or_else(|| screen_rect(self, entity_bounds(entity), canvas_rect));
            let visible_screen = screen.intersect(canvas_clip_rect);
            direct_editor_rects.push(visible_screen);
            if interaction == DirectEditorInteraction::Live
                && ui.ctx().input(|input| {
                    input.pointer.button_pressed(PointerButton::Primary)
                        && input
                            .pointer
                            .interact_pos()
                            .is_some_and(|pointer| visible_screen.contains(pointer))
                })
            {
                self.focused_editor = Some(entity.id);
            }
            let mode = if is_focused {
                ChildMode::Active
            } else {
                ChildMode::Passive
            };
            let Some(handle) = editors.place(ui, block_id, screen, 0.0, 1.0, mode) else {
                continue;
            };
            if is_focused && !handle.active() && editors.is_frame_child(block_id) {
                self.focused_editor = None;
            }
            for change in handle.take_view_changes() {
                match change {
                    ViewChange::Fit => self.fit_entity_requested = Some(entity.id),
                    change => viewport.apply(change),
                }
            }
        }

        let (context_layer_move, keyboard_action) = self.handle_canvas_input(
            &response,
            canvas_rect,
            &entities,
            editors,
            &direct_editor_rects,
        );
        if self.grouped_inspector_edit_active
            && ui.ctx().input(|input| input.pointer.any_released())
        {
            self.block.finish_history_group();
            self.grouped_inspector_edit_active = false;
        }
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
        self.handle_picker(editors);
        if self.focused_editor.is_none() {
            action = action
                .or_else(|| self.selected_block_action(ui.ctx(), canvas_rect, &entities, editors));
        }
        if self.gesture.is_some() {
            ui.ctx().request_repaint();
        }
        self.take_action(action.or(keyboard_action), editors);
    }
}

fn view_scale(editors: &Access) -> f32 {
    editors.host().view_scale().unwrap_or(1.0).max(f32::EPSILON)
}

fn fit_into_view(viewport: &mut DirectEditorViewport, clip: Rect, target: Rect) {
    let available = (clip.size() - Vec2::splat(40.0)).max(Vec2::splat(1.0));
    let factor =
        (available.x / target.width().max(1.0)).min(available.y / target.height().max(1.0));
    viewport.change_zoom(factor, Some(target.center()));
    viewport.pan(clip.center() - target.center());
}
