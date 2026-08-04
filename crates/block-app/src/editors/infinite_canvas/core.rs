use super::*;

impl InfiniteCanvasEditor {
    pub(in crate::editors) fn new(
        block: BlockHandle<InfiniteCanvas>,
        client: &BlockClient,
    ) -> Self {
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            tool: Tool::Select,
            render_scale: 1.0,
            selection: HashSet::new(),
            gesture: None,
            picker: BlockPicker::default(),
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
            fit_preview_region_requested: false,
            grouped_inspector_edit_active: false,
        }
    }

    pub(super) fn record_update(
        &mut self,
        before: Vec<CanvasEntity>,
        after: Vec<CanvasEntity>,
        group: bool,
    ) {
        if before == after || after.is_empty() {
            return;
        }
        let operation = InfiniteCanvasOperation::Update { entities: after };
        if group {
            self.grouped_inspector_edit_active = true;
            self.block.operate_grouped([operation]);
        } else {
            self.grouped_inspector_edit_active = false;
            self.block.finish_history_group();
            self.block.operate(operation);
        }
    }

    pub(super) fn record_action(&mut self, operation: InfiniteCanvasOperation) {
        self.grouped_inspector_edit_active = false;
        self.block.finish_history_group();
        self.block.operate(operation);
    }

    pub(super) fn world_to_screen(&self, point: CanvasPoint, rect: Rect) -> Pos2 {
        rect.center() + Vec2::new(point.x, point.y) * self.render_scale
    }

    pub(super) fn screen_to_world(&self, point: Pos2, rect: Rect) -> CanvasPoint {
        let relative = point - rect.center();
        CanvasPoint::new(
            relative.x / self.render_scale,
            relative.y / self.render_scale,
        )
    }

    pub(super) fn selected_entities(&self, entities: &[CanvasEntity]) -> Vec<CanvasEntity> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .cloned()
            .collect()
    }

    pub(super) fn selected_unlocked_entities(
        &self,
        entities: &[CanvasEntity],
    ) -> Vec<CanvasEntity> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .cloned()
            .collect()
    }

    pub(super) fn selection_has_unlocked(&self, entities: &[CanvasEntity]) -> bool {
        entities
            .iter()
            .any(|entity| self.selection.contains(&entity.id) && !entity.locked)
    }

    pub(super) fn entity_selection_ids(entities: &[CanvasEntity], id: Uuid) -> HashSet<Uuid> {
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

    pub(super) fn select_entity(&mut self, entities: &[CanvasEntity], id: Uuid, additive: bool) {
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

    pub(super) fn selected_bounds(&self, entities: &[CanvasEntity]) -> Option<WorldRect> {
        entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .map(entity_bounds)
            .reduce(WorldRect::union)
    }

    pub(super) fn selected_frame(&self, entities: &[CanvasEntity]) -> Option<SelectionFrame> {
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

    pub(super) fn entity_at(&self, entities: &[CanvasEntity], point: CanvasPoint) -> Option<Uuid> {
        entities
            .iter()
            .rev()
            .find(|entity| hit_entity(entity, point, HIT_RADIUS / self.render_scale))
            .map(|entity| entity.id)
    }

    pub(super) fn add_entity(&mut self, entity: CanvasEntity) {
        let id = entity.id;
        let text = matches!(&entity.kind, CanvasEntityKind::Text { .. });
        let select_after_add = !matches!(
            &entity.kind,
            CanvasEntityKind::Line | CanvasEntityKind::Rectangle | CanvasEntityKind::Pen { .. }
        );
        if text {
            self.editing_text = Some(id);
            self.focus_text_requested = true;
        }
        self.record_action(InfiniteCanvasOperation::Add { entity });
        self.selection.clear();
        if select_after_add {
            self.selection.insert(id);
        }
    }

    pub(super) fn duplicate_selection(&mut self, entities: &[CanvasEntity]) {
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

    pub(super) fn begin_move_gesture(
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

    pub(super) fn group_selection(&mut self, entities: &[CanvasEntity]) {
        if !self.selection_can_group(entities) {
            return;
        }
        let before = self.selected_entities(entities);
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

    pub(super) fn selection_can_group(&self, entities: &[CanvasEntity]) -> bool {
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .collect::<Vec<_>>();
        if selected.len() < 2 {
            return false;
        }
        let Some(group_id) = selected[0].group_id else {
            return true;
        };
        selected
            .iter()
            .any(|entity| entity.group_id != Some(group_id))
    }

    pub(super) fn ungroup_selection(&mut self, entities: &[CanvasEntity]) {
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

    pub(super) fn set_selection_locked(&mut self, entities: &[CanvasEntity], locked: bool) {
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

    pub(super) fn align_selection(&mut self, entities: &[CanvasEntity], alignment: Alignment) {
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

    pub(super) fn distribute_selection(&mut self, entities: &[CanvasEntity], horizontal: bool) {
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

    pub(super) fn execute_command(&mut self, command: CanvasCommand, entities: &[CanvasEntity]) {
        match command {
            CanvasCommand::SelectAll => {
                self.selection = entities.iter().map(|entity| entity.id).collect();
                self.tool = Tool::Select;
            }
            CanvasCommand::InvertSelection => {
                let selected = &self.selection;
                self.selection = entities
                    .iter()
                    .filter(|entity| !selected.contains(&entity.id))
                    .map(|entity| entity.id)
                    .collect();
                self.tool = Tool::Select;
            }
            CanvasCommand::Duplicate => self.duplicate_selection(entities),
            CanvasCommand::Delete => {
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
            CanvasCommand::Lock => self.set_selection_locked(entities, true),
            CanvasCommand::Unlock => self.set_selection_locked(entities, false),
            CanvasCommand::Group => self.group_selection(entities),
            CanvasCommand::Ungroup => self.ungroup_selection(entities),
            CanvasCommand::Reorder(movement) => {
                let ids = entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>();
                if !ids.is_empty() {
                    self.record_action(InfiniteCanvasOperation::Reorder { ids, movement });
                }
            }
            CanvasCommand::Copy => {
                self.copy_selection_to_clipboard(entities);
            }
            CanvasCommand::Cut => {
                if self.copy_selection_to_clipboard(entities) {
                    self.execute_command(CanvasCommand::Delete, entities);
                }
            }
            CanvasCommand::Paste => self.paste_entities_from_clipboard(),
        }
    }

    pub(super) fn can_reorder(&self, entities: &[CanvasEntity], movement: CanvasLayerMove) -> bool {
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .map(|entity| entity.id)
            .collect::<HashSet<_>>();
        if selected.is_empty() {
            return false;
        }
        match movement {
            CanvasLayerMove::BringToFront | CanvasLayerMove::ForwardOne => {
                entities.iter().enumerate().any(|(index, entity)| {
                    selected.contains(&entity.id)
                        && entities[index + 1..]
                            .iter()
                            .any(|later| !selected.contains(&later.id))
                })
            }
            CanvasLayerMove::BackOne | CanvasLayerMove::SendToBack => {
                entities.iter().enumerate().any(|(index, entity)| {
                    selected.contains(&entity.id)
                        && entities[..index]
                            .iter()
                            .any(|earlier| !selected.contains(&earlier.id))
                })
            }
        }
    }

    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    pub(super) fn copy_selection_to_clipboard(&mut self, entities: &[CanvasEntity]) -> bool {
        let entities = self.selected_entities(entities);
        if entities.is_empty() {
            return false;
        }
        let payload = CanvasClipboardPayload { entities };
        let result = serde_json::to_string(&payload)
            .map(|json| format!("{CANVAS_CLIPBOARD_PREFIX}{json}"))
            .map_err(|error| error.to_string())
            .and_then(|text| {
                let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
                clipboard.set_text(text).map_err(|error| error.to_string())
            });
        match result {
            Ok(()) => true,
            Err(error) => {
                self.image_import_error = Some(format!("Could not copy canvas objects: {error}"));
                false
            }
        }
    }

    /// Neither Android nor the browser offers a clipboard this can read and
    /// write synchronously while drawing a frame.
    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    pub(super) fn copy_selection_to_clipboard(&mut self, _entities: &[CanvasEntity]) -> bool {
        false
    }

    #[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
    pub(super) fn paste_entities_from_clipboard(&mut self) {
        let result = (|| {
            let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
            let text = clipboard.get_text().map_err(|error| error.to_string())?;
            let json = text
                .strip_prefix(CANVAS_CLIPBOARD_PREFIX)
                .ok_or_else(|| "Clipboard does not contain canvas objects".to_owned())?;
            serde_json::from_str::<CanvasClipboardPayload>(json).map_err(|error| error.to_string())
        })();
        let payload = match result {
            Ok(payload) => payload,
            Err(error) => {
                if error != "Clipboard does not contain canvas objects" {
                    self.image_import_error =
                        Some(format!("Could not paste canvas objects: {error}"));
                }
                return;
            }
        };
        let duplicates = duplicate_entities(
            payload.entities,
            CanvasPoint::new(IMPORT_CASCADE_OFFSET, IMPORT_CASCADE_OFFSET),
        );
        self.block.finish_history_group();
        self.selection.clear();
        for entity in duplicates {
            self.selection.insert(entity.id);
            self.block.operate(InfiniteCanvasOperation::Add { entity });
        }
        self.tool = Tool::Select;
    }

    #[cfg(any(target_os = "android", target_arch = "wasm32"))]
    pub(super) fn paste_entities_from_clipboard(&mut self) {}

    pub(super) fn edit_selected(
        &mut self,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) -> Option<EditorAction> {
        if self.selection.len() != 1 {
            return None;
        }
        let entity = entities
            .iter()
            .find(|entity| self.selection.contains(&entity.id))?;
        match entity.kind {
            CanvasEntityKind::DirectEditor { block_id, .. } => {
                if editors.direct_editor_interaction(block_id)
                    == Some(DirectEditorInteraction::Playback)
                {
                    let cached = editors.client().cached_block(block_id)?;
                    Some(EditorAction::OpenBlock {
                        id: cached.id,
                        block_type: cached.block_type,
                    })
                } else {
                    self.focused_editor = Some(entity.id);
                    None
                }
            }
            CanvasEntityKind::Text { .. } if !entity.locked => {
                self.editing_text = Some(entity.id);
                self.focus_text_requested = true;
                None
            }
            CanvasEntityKind::Block { block_id } => {
                let cached = editors.client().cached_block(block_id)?;
                Some(EditorAction::OpenBlock {
                    id: cached.id,
                    block_type: cached.block_type,
                })
            }
            _ => None,
        }
    }

    pub(super) fn toggle_selected_block_mode(
        &mut self,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
    ) {
        if self.selection.len() != 1 {
            return;
        }
        let Some(entity) = entities
            .iter()
            .find(|entity| self.selection.contains(&entity.id))
            .cloned()
        else {
            return;
        };
        let updated = match entity.kind {
            CanvasEntityKind::DirectEditor { block_id, .. } => {
                if self.focused_editor == Some(entity.id) {
                    self.focused_editor = None;
                }
                direct_editor_to_preview(&entity, block_id)
            }
            CanvasEntityKind::Block { block_id } => editors
                .direct_editor_intrinsic_size(block_id)
                .map(|intrinsic| preview_to_direct_editor(&entity, block_id, intrinsic)),
            _ => None,
        };
        if let Some(updated) = updated {
            self.record_update(vec![entity], vec![updated], false);
        }
    }

    pub(super) fn add_direct_editor(&mut self, block_id: Uuid, center: CanvasPoint) {
        self.add_direct_editor_sized(block_id, center, CanvasPoint::new(180.0, 100.0));
    }

    pub(super) fn add_direct_editor_sized(
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

    pub(super) fn add_imported_image(
        &mut self,
        editors: &mut EditorAccess<'_>,
        image: ImageBlock,
        center: CanvasPoint,
    ) {
        let id = create_image_block(editors, image, self.block.id());
        self.add_direct_editor(id, center);
    }

    pub(super) fn ensure_dependency_editors(
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

    pub(super) fn selection_defaults_to_proportional(
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

    pub(super) fn selection_allows_rotation(
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

    pub(super) fn selection_resize(
        &self,
        entities: &[CanvasEntity],
        editors: &EditorAccess<'_>,
    ) -> DirectEditorResize {
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id) && !entity.locked)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return DirectEditorResize::None;
        }
        let mut horizontal = true;
        let mut vertical = true;
        for entity in selected {
            let resize = match entity.kind {
                CanvasEntityKind::DirectEditor { block_id, .. } => editors
                    .direct_editor_resize(block_id)
                    .unwrap_or(DirectEditorResize::None),
                _ => DirectEditorResize::Both,
            };
            horizontal &= resize.horizontal();
            vertical &= resize.vertical();
        }
        match (horizontal, vertical) {
            (true, true) => DirectEditorResize::Both,
            (true, false) => DirectEditorResize::Horizontal,
            (false, true) => DirectEditorResize::Vertical,
            (false, false) => DirectEditorResize::None,
        }
    }

    pub(super) fn selection_forces_proportional(
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

    pub(super) fn autosize_direct_editors(
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
            let resize = editors
                .direct_editor_resize(block_id)
                .unwrap_or(DirectEditorResize::None);
            if resize == DirectEditorResize::Both {
                if let Some(layout) = direct_editor_layout(entity) {
                    let content_size = layout.content.size();
                    editors.set_direct_editor_intrinsic_size(
                        block_id,
                        Vec2::new(
                            content_size.x / scale.max(f32::EPSILON),
                            content_size.y / scale.max(f32::EPSILON),
                        ),
                    );
                }
            }
            let intrinsic = if resize == DirectEditorResize::Horizontal {
                let width = direct_editor_layout(entity)
                    .map(|layout| layout.content.size().x / scale.max(f32::EPSILON))
                    .unwrap_or(MIN_SIZE);
                editors.direct_editor_intrinsic_size_for_width(block_id, width)
            } else {
                editors.direct_editor_intrinsic_size(block_id)
            };
            let Some(intrinsic) = intrinsic else {
                continue;
            };
            let intrinsic_size = direct_editor_entity_size(intrinsic, scale);
            let desired = CanvasPoint::new(
                if resize.horizontal() {
                    entity.transform.size.x
                } else {
                    intrinsic_size.x
                },
                if resize.vertical() {
                    entity.transform.size.y
                } else {
                    intrinsic_size.y
                },
            );
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

    pub(super) fn update_selected(
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

    pub(super) fn update_selected_text(
        &mut self,
        entities: &[CanvasEntity],
        mut update: impl FnMut(&mut CanvasTextStyle),
    ) {
        let before = entities
            .iter()
            .filter(|entity| {
                self.selection.contains(&entity.id)
                    && matches!(entity.kind, CanvasEntityKind::Text { .. })
            })
            .cloned()
            .collect::<Vec<_>>();
        let after = before
            .iter()
            .cloned()
            .map(|mut entity| {
                let CanvasEntityKind::Text { text_style, .. } = &mut entity.kind else {
                    unreachable!();
                };
                update(text_style);
                entity
            })
            .collect();
        self.record_update(before, after, true);
    }
}
