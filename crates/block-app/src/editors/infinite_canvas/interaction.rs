use super::*;

impl InfiniteCanvasEditor {
    pub(super) fn handle_picker(
        &mut self,
        context: &egui::Context,
        editors: &mut EditorAccess<'_>,
    ) {
        if let Some(block) =
            self.picker
                .handle(context, editors, BlockParent::Uuid(self.block.id()))
        {
            editors.set_parent(block.id, BlockParent::Uuid(self.block.id()));
            if let Some(center) = self.pending_block_center.take() {
                self.add_direct_editor(block.id, center);
                self.tool = Tool::Select;
            } else {
                self.armed_block = Some(CachedBlock {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
                    name: block.name,
                });
                self.tool = Tool::Block;
            }
        }
    }

    pub(super) fn import_dropped_images(
        &mut self,
        response: &egui::Response,
        canvas_rect: Rect,
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
                self.pending_file_drop_position = Some(self.screen_to_world(position, canvas_rect));
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
                    .map(|position| self.screen_to_world(position, canvas_rect))
            })
            .unwrap_or_else(|| self.screen_to_world(response.rect.center(), canvas_rect));
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

    pub(super) fn import_clipboard_image(
        &mut self,
        response: &egui::Response,
        canvas_rect: Rect,
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
        let center = self.screen_to_world(screen_position, canvas_rect);
        self.add_imported_image(editors, image, center);
    }

    pub(super) fn handle_zoom_and_pan(
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

    pub(super) fn update_cursor(
        &self,
        response: &egui::Response,
        world: CanvasPoint,
        canvas_rect: Rect,
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
                let resize = self.selection_resize(entities, editors);
                if self.selection_has_unlocked(entities)
                    && self.selection_allows_rotation(entities, editors)
                    && frame.is_some_and(|frame| {
                        rotate_handle_at(self, frame, canvas_rect)
                            .distance(self.world_to_screen(world, canvas_rect))
                            <= HANDLE_RADIUS + 3.0
                    })
                {
                    egui::CursorIcon::Grab
                } else if let Some(handle) = self
                    .selection_has_unlocked(entities)
                    .then(|| frame)
                    .flatten()
                    .and_then(|frame| resize_handle_at(self, frame, canvas_rect, world, resize))
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

    pub(super) fn handle_canvas_input(
        &mut self,
        response: &egui::Response,
        canvas_rect: Rect,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
        direct_editor_rects: &[Rect],
        viewport: &mut DirectEditorViewport,
    ) -> (Option<CanvasLayerMove>, Option<EditorAction>) {
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
                    }
                }
            }
            if modifiers.command && response.ctx.input(|input| input.key_pressed(egui::Key::A)) {
                let command = if modifiers.shift {
                    CanvasCommand::InvertSelection
                } else {
                    CanvasCommand::SelectAll
                };
                self.execute_command(command, entities);
            }
            if modifiers.command && response.ctx.input(|input| input.key_pressed(egui::Key::D)) {
                self.execute_command(CanvasCommand::Duplicate, entities);
            }
            for (key, command) in [
                (egui::Key::C, CanvasCommand::Copy),
                (egui::Key::X, CanvasCommand::Cut),
                (egui::Key::V, CanvasCommand::Paste),
            ] {
                if modifiers.command && response.ctx.input(|input| input.key_pressed(key)) {
                    self.execute_command(command, entities);
                }
            }
            if modifiers.command
                && response
                    .ctx
                    .input(|input| input.key_pressed(egui::Key::OpenBracket))
            {
                self.execute_command(CanvasCommand::Reorder(CanvasLayerMove::BackOne), entities);
            }
            if modifiers.command
                && response
                    .ctx
                    .input(|input| input.key_pressed(egui::Key::CloseBracket))
            {
                self.execute_command(
                    CanvasCommand::Reorder(CanvasLayerMove::ForwardOne),
                    entities,
                );
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
                keyboard_action = self.edit_selected(entities, editors);
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
            self.execute_command(CanvasCommand::Delete, entities);
        }

        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.ctx.pointer_hover_pos());
        if pointer.is_some_and(|pointer| {
            direct_editor_rects
                .iter()
                .any(|rect| rect.contains(pointer))
        }) {
            return (None, keyboard_action);
        }

        if self.handle_zoom_and_pan(response, viewport) {
            return (None, keyboard_action);
        }

        let world = pointer.map(|point| self.screen_to_world(point, canvas_rect));

        if response.hovered() {
            if let Some(world) = world {
                self.update_cursor(response, world, canvas_rect, entities, editors);
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
                    editors.set_parent(dragged.reference.id, BlockParent::Uuid(self.block.id()));
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
        response.context_menu(|ui| {
            if self.context_menu_for_selection {
                if ui.button("Open / edit").clicked() {
                    keyboard_action = self.edit_selected(entities, editors);
                    ui.close();
                }
                let block_mode = self.selection.len() == 1
                    && entities.iter().any(|entity| {
                        self.selection.contains(&entity.id)
                            && matches!(
                                entity.kind,
                                CanvasEntityKind::Block { .. }
                                    | CanvasEntityKind::DirectEditor { .. }
                            )
                    });
                if ui
                    .add_enabled(
                        block_mode,
                        egui::Button::new("Toggle preview / direct editor"),
                    )
                    .clicked()
                {
                    self.toggle_selected_block_mode(entities, editors);
                    ui.close();
                }
                if ui.button("Fit selection").clicked() {
                    self.fit_selection_requested = true;
                    ui.close();
                }
                ui.separator();
                for (label, command) in [
                    ("Cut", CanvasCommand::Cut),
                    ("Copy", CanvasCommand::Copy),
                    ("Paste", CanvasCommand::Paste),
                    ("Duplicate", CanvasCommand::Duplicate),
                    ("Delete", CanvasCommand::Delete),
                ] {
                    if ui.button(label).clicked() {
                        self.execute_command(command, entities);
                        ui.close();
                    }
                }
                ui.separator();
                let selected = entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id))
                    .collect::<Vec<_>>();
                let can_group = self.selection_can_group(entities);
                let can_ungroup = selected.iter().any(|entity| entity.group_id.is_some());
                let can_lock = selected.iter().any(|entity| !entity.locked);
                let can_unlock = selected.iter().any(|entity| entity.locked);
                for (label, enabled, command) in [
                    ("Group", can_group, CanvasCommand::Group),
                    ("Ungroup", can_ungroup, CanvasCommand::Ungroup),
                    ("Lock", can_lock, CanvasCommand::Lock),
                    ("Unlock", can_unlock, CanvasCommand::Unlock),
                ] {
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        self.execute_command(command, entities);
                        ui.close();
                    }
                }
                ui.separator();
                for (label, movement) in [
                    ("Bring to front", CanvasLayerMove::BringToFront),
                    ("Forward", CanvasLayerMove::ForwardOne),
                    ("Backward", CanvasLayerMove::BackOne),
                    ("Send to back", CanvasLayerMove::SendToBack),
                ] {
                    if ui
                        .add_enabled(
                            self.can_reorder(entities, movement),
                            egui::Button::new(label),
                        )
                        .clicked()
                    {
                        layer_move = Some(movement);
                        ui.close();
                    }
                }
                ui.separator();
                if ui.button("Select all").clicked() {
                    self.execute_command(CanvasCommand::SelectAll, entities);
                    ui.close();
                }
                if ui.button("Invert selection").clicked() {
                    self.execute_command(CanvasCommand::InvertSelection, entities);
                    ui.close();
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
                    if ui.button("Text").clicked() {
                        if let Some(center) = self.context_menu_position {
                            self.add_entity(CanvasEntity {
                                id: Uuid::new_v4(),
                                transform: CanvasTransform::new(
                                    center,
                                    CanvasPoint::new(180.0, 36.0),
                                    0.0,
                                ),
                                kind: CanvasEntityKind::Text {
                                    text: String::new(),
                                    text_style: CanvasTextStyle::default(),
                                },
                                style: CanvasEntityStyle::default(),
                                group_id: None,
                                locked: false,
                            });
                        }
                        self.tool = Tool::Select;
                        ui.close();
                    }
                    if ui.button("Freehand").clicked() {
                        self.tool = Tool::Pen;
                        self.armed_block = None;
                        ui.close();
                    }
                    if ui.button("Image…").clicked() {
                        match pick_image_file() {
                            Ok(Some(image)) => {
                                self.image_import_error = None;
                                let center = self.context_menu_position.unwrap_or_default();
                                self.add_imported_image(editors, image, center);
                                self.tool = Tool::Select;
                            }
                            Ok(None) => {}
                            Err(error) => self.image_import_error = Some(error),
                        }
                        ui.close();
                    }
                    ui.menu_button("Block", |ui| {
                        self.pending_block_center = self.context_menu_position;
                        self.picker
                            .show_menu_excluding(ui, editors.registry(), [self.block.id()]);
                    });
                });
                ui.separator();
                if ui.button("Paste").clicked() {
                    self.execute_command(CanvasCommand::Paste, entities);
                    ui.close();
                }
                if ui.button("Select all").clicked() {
                    self.execute_command(CanvasCommand::SelectAll, entities);
                    ui.close();
                }
                if ui.button("Invert selection").clicked() {
                    self.execute_command(CanvasCommand::InvertSelection, entities);
                    ui.close();
                }
            }
        });

        let Some(world) = world else {
            return (layer_move, keyboard_action);
        };
        if self.tool == Tool::Select && response.hovered() && response.double_clicked() {
            if let Some(id) = self.entity_at(entities, world) {
                let entity = entities.iter().find(|entity| entity.id == id).unwrap();
                if matches!(entity.kind, CanvasEntityKind::Text { .. }) && !entity.locked {
                    self.select_entity(entities, id, false);
                    self.editing_text = Some(id);
                    self.focus_text_requested = true;
                    self.gesture = None;
                    return (layer_move, keyboard_action);
                }
            }
        }
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
                    let resize = self.selection_resize(entities, editors);
                    let handle = has_unlocked
                        .then(|| selected_frame)
                        .flatten()
                        .and_then(|frame| {
                            resize_handle_at(self, frame, canvas_rect, world, resize)
                        });
                    let rotate = has_unlocked
                        && self.selection_allows_rotation(entities, editors)
                        && selected_frame.is_some_and(|frame| {
                            rotate_handle_at(self, frame, canvas_rect)
                                .distance(self.world_to_screen(world, canvas_rect))
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
                        let scale_text = response.ctx.input(|input| input.modifiers.alt);
                        let preserve_aspect_ratio = scale_text
                            || force_preserve_aspect_ratio
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
                            scale_text,
                        });
                    } else if let Some(id) = self.entity_at(entities, world) {
                        let entity = entities.iter().find(|entity| entity.id == id).unwrap();
                        if let CanvasEntityKind::DirectEditor { block_id, .. } = entity.kind {
                            let content = direct_editor_layout(entity)
                                .is_some_and(|layout| layout.content.contains(world));
                            let live = editors.direct_editor_interaction(block_id)
                                == Some(DirectEditorInteraction::Live);
                            if content && live {
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
                            return (layer_move, keyboard_action);
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
                            pointer: world,
                            from_center: response.ctx.input(|input| input.modifiers.alt),
                            additive,
                        });
                    }
                }
                Tool::Line | Tool::Rectangle | Tool::Text => {
                    self.gesture = Some(Gesture::Create {
                        tool: self.tool,
                        start: world,
                        current: world,
                        pointer: world,
                        from_center: response.ctx.input(|input| input.modifiers.alt),
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
                        editors.set_parent(block.id, BlockParent::Uuid(self.block.id()));
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
                    pointer,
                    from_center,
                }) => {
                    let modifiers = response.ctx.input(|input| input.modifiers);
                    if *tool == Tool::Rectangle {
                        let delta = CanvasPoint::new(world.x - pointer.x, world.y - pointer.y);
                        if modifiers.ctrl {
                            start.x += delta.x;
                            start.y += delta.y;
                        } else {
                            current.x += delta.x;
                            current.y += delta.y;
                        }
                        *pointer = world;
                        *from_center = modifiers.alt;
                    } else {
                        *current = if *tool == Tool::Line && modifiers.shift {
                            constrain_point_angle(*start, world, std::f32::consts::FRAC_PI_4)
                        } else {
                            world
                        };
                    }
                }
                Some(Gesture::SelectBox {
                    start,
                    current,
                    pointer,
                    from_center,
                    ..
                }) => {
                    let modifiers = response.ctx.input(|input| input.modifiers);
                    let delta = CanvasPoint::new(world.x - pointer.x, world.y - pointer.y);
                    if modifiers.ctrl {
                        start.x += delta.x;
                        start.y += delta.y;
                    } else {
                        current.x += delta.x;
                        current.y += delta.y;
                    }
                    *pointer = world;
                    *from_center = modifiers.alt;
                }
                Some(Gesture::Move { current, .. }) => {
                    *current = world;
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
                    scale_text,
                    ..
                }) => {
                    *current = world;
                    *scale_text = response.ctx.input(|input| input.modifiers.alt);
                    *preserve_aspect_ratio = *scale_text
                        || *force_preserve_aspect_ratio
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
        (layer_move, keyboard_action)
    }

    pub(super) fn finish_gesture(&mut self, gesture: Gesture, entities: &[CanvasEntity]) {
        match gesture {
            Gesture::Create {
                tool,
                start,
                current,
                from_center,
                ..
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
                        let bounds = gesture_rect(start, current, from_center);
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
                from_center,
                additive,
                ..
            } => {
                if !additive {
                    self.selection.clear();
                }
                if distance(start, current) < HIT_RADIUS / self.render_scale {
                    return;
                }
                let selection = gesture_rect(start, current, from_center);
                let hits = entities
                    .iter()
                    .filter(|entity| selection.contains_rect(entity_bounds(entity)))
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

    pub(super) fn paint(
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
            paint_entity(
                self,
                painter,
                rect,
                entity,
                dependency_details,
                editors,
                true,
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
                true,
                1.0,
            );
        }

        if let Some(region) = self.block.read().and_then(|canvas| canvas.preview_region()) {
            let preview_rect = screen_rect(self, preview_region_bounds(region), rect);
            painter.rect_stroke(
                preview_rect,
                0.0,
                Stroke::new(1.5, Color32::from_rgb(245, 180, 60)),
                egui::StrokeKind::Inside,
            );
            painter.text(
                preview_rect.left_top() + Vec2::new(5.0, 4.0),
                egui::Align2::LEFT_TOP,
                "Preview",
                egui::FontId::proportional(12.0),
                Color32::from_rgb(245, 180, 60),
            );
        }

        if let Some(Gesture::Create {
            tool,
            start,
            current,
            from_center,
            ..
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
                    let selection = if *tool == Tool::Rectangle {
                        gesture_rect(*start, *current, *from_center)
                    } else {
                        WorldRect::from_points(*start, *current)
                    };
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
        if let Some(Gesture::SelectBox {
            start,
            current,
            from_center,
            ..
        }) = &self.gesture
        {
            let selection = screen_rect(self, gesture_rect(*start, *current, *from_center), rect);
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
                self.selection_resize(entities, editors),
                self.selection_has_unlocked(entities)
                    && self.selection_allows_rotation(entities, editors),
            );
        }

        let tool_icon = match self.tool {
            Tool::Select => None,
            Tool::Line => Some(ICON_DIAGONAL_LINE),
            Tool::Rectangle => Some(ICON_RECTANGLE),
            Tool::Text => Some(ICON_TEXT_FIELDS),
            Tool::Pen => Some(ICON_DRAW),
            Tool::Block => Some(ICON_DATA_OBJECT),
        };
        let pointer = painter.ctx().pointer_hover_pos();
        let panning = painter
            .ctx()
            .input(|input| input.key_down(egui::Key::Space));
        if let (Some(icon), Some(pointer)) = (tool_icon, pointer) {
            if painter.clip_rect().contains(pointer) && !panning && self.focused_editor.is_none() {
                let center = pointer + Vec2::new(0.0, 20.0);
                let badge = Rect::from_center_size(center, Vec2::splat(22.0));
                let visuals = &painter.ctx().global_style().visuals;
                painter.rect(
                    badge,
                    5.0,
                    visuals.panel_fill,
                    visuals.widgets.noninteractive.bg_stroke,
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    icon.codepoint,
                    egui::FontId::new(
                        16.0,
                        egui::FontFamily::Name(egui_material_icons::FONT_FAMILY.into()),
                    ),
                    visuals.text_color(),
                );
            }
        }
    }

    pub(super) fn selected_frame_with_preview(
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

    pub(super) fn selected_block_action(
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
        let bounds = entity_bounds(entity);
        let position =
            self.world_to_screen(CanvasPoint::new(bounds.center().x, bounds.max.y), rect);
        let (label, cached, enters_editor) = match entity.kind {
            CanvasEntityKind::Block { block_id } => {
                let cached = editors.client().cached_block(block_id);
                let label = cached.as_ref().map_or_else(
                    || "Loading…".to_owned(),
                    |block| editors.registry().icon_label(block.block_type, &block.name),
                );
                (label, cached, false)
            }
            CanvasEntityKind::DirectEditor { block_id, .. }
                if editors.direct_editor_interaction(block_id)
                    == Some(DirectEditorInteraction::Playback) =>
            {
                let cached = editors.client().cached_block(block_id);
                ("Open presentation".to_owned(), cached, false)
            }
            CanvasEntityKind::DirectEditor { block_id, .. }
                if editors.direct_editor_interaction(block_id)
                    == Some(DirectEditorInteraction::Preview) =>
            {
                ("Edit".to_owned(), None, true)
            }
            _ => return None,
        };
        let mut action = None;
        egui::Area::new(egui::Id::new(("open-canvas-block", entity.id)))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(position + Vec2::new(0.0, 6.0))
            .show(context, |ui| {
                if ui
                    .add_enabled(enters_editor || cached.is_some(), egui::Button::new(label))
                    .on_disabled_hover_text("Waiting for cached block metadata")
                    .clicked()
                {
                    if enters_editor {
                        self.focused_editor = Some(entity.id);
                    } else {
                        let cached = cached.as_ref().unwrap();
                        action = Some(EditorAction::OpenBlock {
                            id: cached.id,
                            block_type: cached.block_type,
                        });
                    }
                }
            });
        action
    }
}
