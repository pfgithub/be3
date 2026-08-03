use super::*;

impl BlockEditor for InfiniteCanvasEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let Some(canvas) = self.block.read() else {
            return false;
        };
        let entities = canvas.entities().to_vec();
        let preview_region = canvas
            .preview_region()
            .unwrap_or_else(|| preview_region_for_entities(&entities));
        drop(canvas);

        let intrinsic = Vec2::new(preview_region.size.x, preview_region.size.y);
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
        let rect = Rect::from_center_size(
            center
                - Vec2::new(preview_region.center.x, preview_region.center.y) * self.render_scale,
            Vec2::new(width, height),
        );
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
                false,
                context.opacity,
            );
        }
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        if self
            .block
            .read()
            .is_some_and(|canvas| canvas.preview_region().is_some())
        {
            DirectEditorResize::Both
        } else {
            DirectEditorResize::None
        }
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    fn direct_editor_handles_viewport_input(&self, _editors: &EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let canvas = self.block.read()?;
        let region = canvas
            .preview_region()
            .unwrap_or_else(|| preview_region_for_entities(canvas.entities()));
        Some(Vec2::new(region.size.x, region.size.y))
    }

    fn set_direct_editor_intrinsic_size(
        &mut self,
        size: Vec2,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        let Some(region) = self.block.read().and_then(|canvas| canvas.preview_region()) else {
            return false;
        };
        let updated = CanvasPreviewRegion::new(
            region.center,
            CanvasPoint::new(size.x.max(MIN_SIZE), size.y.max(MIN_SIZE)),
        );
        if (updated.size.x - region.size.x).abs() < 0.01
            && (updated.size.y - region.size.y).abs() < 0.01
        {
            return false;
        }
        self.record_action(InfiniteCanvasOperation::SetPreviewRegion {
            region: Some(updated),
        });
        true
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
            self.show_toolbar(ui, &entities, editors, viewport);
        }
        if let Some(error) = self.image_import_error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.image_import_error = None;
                }
            });
        }
        action
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
        focused_direct_editor(self.focused_editor, canvas.entities())
            .is_none_or(|focused| editors.direct_editor_has_right_sidebar(focused.1))
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

    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let previous = viewport.replace_content_rect(None);
        let action = self.direct_editor_ui(ui, editors, scale, viewport);
        viewport.replace_content_rect(previous);
        action
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
        let canvas_rect = viewport.content_rect().unwrap_or(response.rect);
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
        if std::mem::take(&mut self.fit_preview_region_requested) {
            if let Some(region) = self.block.read().and_then(|canvas| canvas.preview_region()) {
                let preview = screen_rect(self, preview_region_bounds(region), canvas_rect);
                let available = (canvas_clip_rect.size() - Vec2::splat(40.0)).max(Vec2::splat(1.0));
                let factor = (available.x / preview.width().max(1.0))
                    .min(available.y / preview.height().max(1.0));
                viewport.change_zoom(factor, Some(preview.center()));
                viewport.pan(canvas_clip_rect.center() - preview.center());
                viewport.resume_auto_fit();
            }
        }
        if self.focused_editor.is_none() {
            self.import_dropped_images(&response, canvas_rect, editors);
            self.import_clipboard_image(&response, canvas_rect, editors);
        }
        self.paint(
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
            );
        for entity in displayed_entities {
            let CanvasEntityKind::DirectEditor { block_id, scale } = entity.kind else {
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
            let embedded = embedded_editor_ui(
                ui,
                editors,
                block_id,
                ("canvas-direct-editor", entity.id),
                screen,
                visible_screen,
                scale * self.render_scale,
                viewport,
            );
            action = action.or(embedded);
        }

        let (context_layer_move, keyboard_action) = self.handle_canvas_input(
            &response,
            canvas_rect,
            &entities,
            editors,
            &direct_editor_rects,
            viewport,
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
        self.handle_picker(ui.ctx(), editors);
        if self.focused_editor.is_none() {
            action = action
                .or_else(|| self.selected_block_action(ui.ctx(), canvas_rect, &entities, editors));
        }
        if self.gesture.is_some() {
            ui.ctx().request_repaint();
        }
        action.or(keyboard_action)
    }
}
