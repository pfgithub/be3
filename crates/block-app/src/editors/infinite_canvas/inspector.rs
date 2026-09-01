use super::*;

impl InfiniteCanvasEditor {
    pub(super) fn show_preview_region_inspector(
        &mut self,
        ui: &mut egui::Ui,
        entities: &[CanvasEntity],
    ) {
        let preview_region = self.block.read().and_then(|canvas| canvas.preview_region());
        egui::CollapsingHeader::new("Canvas preview")
            .default_open(preview_region.is_some())
            .show(ui, |ui| {
                let mut enabled = preview_region.is_some();
                if ui.checkbox(&mut enabled, "Use preview region").changed() {
                    let region = enabled.then(|| preview_region_for_entities(entities));
                    self.record_action(InfiniteCanvasOperation::SetPreviewRegion { region });
                }
                let Some(mut region) = preview_region else {
                    ui.weak("Without a region, previews use the canvas content extents.");
                    return;
                };
                let mut changed = false;
                egui::Grid::new("canvas-preview-region-fields")
                    .num_columns(4)
                    .show(ui, |ui| {
                        ui.label("X");
                        changed |= ui
                            .add(egui::DragValue::new(&mut region.center.x).speed(1.0))
                            .changed();
                        ui.label("Y");
                        changed |= ui
                            .add(egui::DragValue::new(&mut region.center.y).speed(1.0))
                            .changed();
                        ui.end_row();
                        ui.label("W");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut region.size.x)
                                    .speed(1.0)
                                    .range(MIN_SIZE..=f32::INFINITY),
                            )
                            .changed();
                        ui.label("H");
                        changed |= ui
                            .add(
                                egui::DragValue::new(&mut region.size.y)
                                    .speed(1.0)
                                    .range(MIN_SIZE..=f32::INFINITY),
                            )
                            .changed();
                        ui.end_row();
                    });
                if changed {
                    self.grouped_inspector_edit_active = true;
                    self.block
                        .operate_grouped([InfiniteCanvasOperation::SetPreviewRegion {
                            region: Some(region),
                        }]);
                }
                if ui.button("Fit region to content").clicked() {
                    self.record_action(InfiniteCanvasOperation::SetPreviewRegion {
                        region: Some(preview_region_for_entities(entities)),
                    });
                }
            });
    }

    fn show_components_inspector(
        &mut self,
        ui: &mut egui::Ui,
        selected: &[&CanvasEntity],
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let selected_ids = self.selection.clone();
        let mut schemas = Vec::new();
        let mut seen = HashSet::new();
        for entity in selected {
            for component in &entity.components {
                if seen.insert(component.schema_id) {
                    schemas.push(component.schema_id);
                }
            }
        }
        let dependency_labels = self
            .dependencies
            .read()
            .into_iter()
            .map(|reference| {
                (
                    reference.id,
                    BlockLabel::for_reference(editors.registry(), &reference),
                )
            })
            .collect::<HashMap<_, _>>();
        let component_references = selected
            .iter()
            .flat_map(|entity| &entity.components)
            .flat_map(|component| component.values.values())
            .filter_map(|value| match value {
                DatabaseValue::Block(reference) => Some(*reference),
                _ => None,
            })
            .collect::<HashSet<_>>();
        let block_labels = component_references
            .into_iter()
            .filter_map(|reference| {
                self.resolve_block_id(editors, reference)
                    .and_then(|id| dependency_labels.get(&id).cloned())
                    .map(|label| (reference, label))
            })
            .collect::<HashMap<_, _>>();

        let mut action = None;
        egui::CollapsingHeader::new("Components")
            .default_open(true)
            .show(ui, |ui| {
                for schema_id in schemas {
                    let attached_count = selected
                        .iter()
                        .filter(|entity| {
                            entity
                                .components
                                .iter()
                                .any(|component| component.schema_id == schema_id)
                        })
                        .count();
                    let resolved_id = self.resolve_block_id(editors, schema_id);
                    let schema =
                        resolved_id.map(|id| editors.client().get_block::<DatabaseSchema>(id));
                    let fields = schema
                        .as_ref()
                        .and_then(BlockHandle::read)
                        .map(|schema| schema.fields().to_vec());

                    if let (Some(schema), Some(_)) = (schema.as_ref(), fields.as_ref()) {
                        ui.label(
                            BlockLabel::for_handle(editors.registry(), schema)
                                .widget_text(ui.style()),
                        );
                    } else {
                        ui.strong("Loading schema...");
                    }
                    if attached_count != selected.len() {
                        ui.weak(format!(
                            "Attached to {attached_count} of {}",
                            selected.len()
                        ));
                    }

                    let mut add_to_all = false;
                    let mut remove = false;
                    ui.horizontal_wrapped(|ui| {
                        if attached_count != selected.len() && ui.button("Add to all").clicked() {
                            add_to_all = true;
                        }
                        if ui
                            .add_enabled(resolved_id.is_some(), egui::Button::new("Open schema"))
                            .clicked()
                        {
                            action = resolved_id.map(|id| EditorAction::OpenBlock {
                                id,
                                block_type: <DatabaseSchema as block::Block>::TYPE_ID,
                            });
                        }
                        if ui.button("Remove").clicked() {
                            remove = true;
                        }
                    });

                    let output = if attached_count == selected.len() {
                        if let (Some(fields), Some(schema_uuid)) = (fields.as_deref(), resolved_id)
                        {
                            let values = selected
                                .iter()
                                .map(|entity| {
                                    &entity
                                        .components
                                        .iter()
                                        .find(|component| component.schema_id == schema_id)
                                        .unwrap()
                                        .values
                                })
                                .collect::<Vec<_>>();
                            self.component_editors.entry(schema_id).or_default().ui(
                                ui,
                                fields,
                                &values,
                                &block_labels,
                                &format!("infinite-canvas.component.{schema_uuid}"),
                            )
                        } else {
                            DatabaseValueEditorOutput::default()
                        }
                    } else {
                        DatabaseValueEditorOutput::default()
                    };

                    let before = selected
                        .iter()
                        .map(|entity| (*entity).clone())
                        .collect::<Vec<_>>();
                    if remove {
                        let mut after = before.clone();
                        remove_component(&mut after, &selected_ids, schema_id);
                        self.record_update(before, after, false);
                    } else if add_to_all {
                        let mut after = before.clone();
                        attach_component(&mut after, &selected_ids, schema_id);
                        self.record_update(before, after, false);
                    } else if !output.changes.is_empty() {
                        let group = output.changes.iter().all(|change| change.continuous);
                        let mut after = before.clone();
                        for change in output.changes {
                            set_component_value(
                                &mut after,
                                &selected_ids,
                                schema_id,
                                change.field_id,
                                change.value,
                            );
                        }
                        self.record_update(before, after, group);
                    }
                    if let Some(request) = output.block_pick {
                        let mut entity_ids = selected_ids.iter().copied().collect::<Vec<_>>();
                        entity_ids.sort_unstable();
                        self.pending_value_target = Some(PendingComponentValuePick {
                            schema_id,
                            field_id: request.field_id,
                            entity_ids,
                        });
                        if let Some(block_type) = request.block_type {
                            self.value_picker.open_for_types([], [block_type]);
                        } else {
                            self.value_picker.open([]);
                        }
                    }
                    ui.add_space(8.0);
                }

                if ui.button("Add component...").clicked() {
                    self.pending_component_entities =
                        Some(selected.iter().map(|entity| entity.id).collect());
                    self.component_picker
                        .open_for_types([], [<DatabaseSchema as block::Block>::TYPE_ID]);
                }
            });
        action
    }

    pub(super) fn show_inspector(
        &mut self,
        ui: &mut egui::Ui,
        entities: &[CanvasEntity],
        editors: &mut EditorAccess<'_>,
        show_heading: bool,
    ) -> (Option<CanvasLayerMove>, Option<EditorAction>) {
        if show_heading {
            ui.heading("Inspector");
            ui.separator();
        }
        let mut component_selection = self.selection.iter().copied().collect::<Vec<_>>();
        component_selection.sort_unstable();
        if self.component_editor_selection != component_selection {
            self.component_editor_selection = component_selection;
            self.component_editors.clear();
        }
        let selected = entities
            .iter()
            .filter(|entity| self.selection.contains(&entity.id))
            .collect::<Vec<_>>();
        self.show_preview_region_inspector(ui, entities);
        ui.separator();
        if selected.is_empty() {
            ui.weak("Select an object to edit its appearance.");
            return (None, None);
        }

        let selection_label = match selected.as_slice() {
            [entity] => format!(
                "{} selected{}",
                entity_kind_label(&entity.kind),
                if entity.locked { " · Locked" } else { "" }
            ),
            _ if selected
                .first()
                .and_then(|entity| entity.group_id)
                .is_some_and(|group| {
                    selected.iter().all(|entity| entity.group_id == Some(group))
                }) =>
            {
                format!("Group · {} objects", selected.len())
            }
            _ => format!("{} objects selected", selected.len()),
        };
        ui.weak(selection_label);

        egui::CollapsingHeader::new("Transform")
            .default_open(true)
            .show(ui, |ui| {
                if let [entity] = selected.as_slice() {
                    let transform_enabled = !entity.locked;
                    let resize = match entity.kind {
                        CanvasEntityKind::DirectEditor { block_id, .. } => self
                            .resolve_block_id(editors, block_id)
                            .and_then(|id| editors.direct_editor_resize(id))
                            .unwrap_or(DirectEditorResize::None),
                        _ => DirectEditorResize::Both,
                    };
                    let mut updated = (*entity).clone();
                    let mut changed = false;
                    egui::Grid::new("canvas-transform-fields")
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.label("X");
                            changed |= ui
                                .add_enabled(
                                    transform_enabled,
                                    egui::DragValue::new(&mut updated.transform.center.x)
                                        .speed(1.0),
                                )
                                .changed();
                            ui.label("Y");
                            changed |= ui
                                .add_enabled(
                                    transform_enabled,
                                    egui::DragValue::new(&mut updated.transform.center.y)
                                        .speed(1.0),
                                )
                                .changed();
                            ui.end_row();

                            let original_size = updated.transform.size;
                            let mut width = original_size.x;
                            let mut height = original_size.y;
                            ui.label("W");
                            let width_changed = ui
                                .add_enabled(
                                    transform_enabled && resize.horizontal(),
                                    egui::DragValue::new(&mut width)
                                        .speed(1.0)
                                        .range(MIN_SIZE..=f32::INFINITY),
                                )
                                .changed();
                            ui.label("H");
                            let height_changed = ui
                                .add_enabled(
                                    transform_enabled && resize.vertical(),
                                    egui::DragValue::new(&mut height)
                                        .speed(1.0)
                                        .range(MIN_SIZE..=f32::INFINITY),
                                )
                                .changed();
                            ui.end_row();
                            if width_changed {
                                updated.transform.size.x = width;
                                changed = true;
                            }
                            if height_changed {
                                updated.transform.size.y = height;
                                changed = true;
                            }

                            let mut degrees = updated.transform.rotation.to_degrees();
                            ui.label("Rotation");
                            let rotation = ui.add_enabled(
                                transform_enabled
                                    && self.selection_allows_rotation(entities, editors),
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
            });

        if let [entity] = selected.as_slice() {
            let block_mode = match entity.kind {
                CanvasEntityKind::Block { block_id } => Some((block_id, false)),
                CanvasEntityKind::DirectEditor { block_id, .. } => Some((block_id, true)),
                _ => None,
            };
            if let Some((block_id, direct)) = block_mode {
                let resolved_id = self.resolve_block_id(editors, block_id);
                egui::CollapsingHeader::new("Block")
                    .default_open(true)
                    .show(ui, |ui| {
                        let available = direct
                            || resolved_id
                                .is_some_and(|id| editors.direct_editor_capabilities(id).is_some());
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
                                resolved_id
                                    .and_then(|id| editors.direct_editor_intrinsic_size(id))
                                    .map(|intrinsic| {
                                        preview_to_direct_editor(entity, block_id, intrinsic)
                                    })
                            };
                            if let Some(updated) = updated {
                                self.record_update(vec![(*entity).clone()], vec![updated], true);
                            }
                        }
                        if let CanvasEntityKind::DirectEditor { scale, .. } = entity.kind {
                            let mut updated_scale = scale;
                            ui.horizontal(|ui| {
                                ui.label("Scale");
                                if ui
                                    .add_enabled(
                                        !entity.locked,
                                        egui::DragValue::new(&mut updated_scale)
                                            .range(0.1..=8.0)
                                            .speed(0.01)
                                            .custom_formatter(|value, _| {
                                                format!("{:.0}%", value * 100.0)
                                            })
                                            .custom_parser(|text| {
                                                text.trim_end_matches('%')
                                                    .parse::<f64>()
                                                    .ok()
                                                    .map(|value| value / 100.0)
                                            }),
                                    )
                                    .changed()
                                {
                                    let factor = updated_scale / scale.max(f32::EPSILON);
                                    let mut updated = (*entity).clone();
                                    if let CanvasEntityKind::DirectEditor { scale, .. } =
                                        &mut updated.kind
                                    {
                                        *scale = updated_scale;
                                    }
                                    updated.transform.size.x *= factor;
                                    updated.transform.size.y *= factor;
                                    self.record_update(
                                        vec![(*entity).clone()],
                                        vec![updated],
                                        true,
                                    );
                                }
                            });
                        }
                    });
            }
        }

        let editor_action = self.show_components_inspector(ui, &selected, editors);

        egui::CollapsingHeader::new("Appearance")
            .default_open(true)
            .show(ui, |ui| {
                let foreground = common_value(
                    selected
                        .iter()
                        .filter(|entity| {
                            !matches!(
                                entity.kind,
                                CanvasEntityKind::Block { .. }
                                    | CanvasEntityKind::DirectEditor { .. }
                            )
                        })
                        .map(|entity| entity.style.foreground),
                );
                if !matches!(foreground, CommonValue::None) {
                    if let Some(color) = color_menu(ui, "Color", foreground) {
                        self.remember_foreground(color);
                        self.update_selected(
                            entities,
                            |kind| {
                                !matches!(
                                    kind,
                                    CanvasEntityKind::Block { .. }
                                        | CanvasEntityKind::DirectEditor { .. }
                                )
                            },
                            |style| style.foreground = color,
                        );
                    }
                }

                let stroked = selected.iter().copied().filter(|entity| {
                    matches!(
                        entity.kind,
                        CanvasEntityKind::Line
                            | CanvasEntityKind::Rectangle
                            | CanvasEntityKind::Pen { .. }
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
                        self.remember_fill(fill);
                        self.update_selected(
                            entities,
                            |kind| matches!(kind, CanvasEntityKind::Rectangle),
                            |style| style.fill = fill,
                        );
                    }

                    let radius =
                        common_value(rectangles.iter().map(|entity| entity.style.corner_radius));
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

                let texts = selected
                    .iter()
                    .copied()
                    .filter_map(|entity| match &entity.kind {
                        CanvasEntityKind::Text { text_style, .. } => Some(*text_style),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if !texts.is_empty() {
                    ui.separator();
                    ui.strong("Text");

                    if let [entity] = selected.as_slice() {
                        if let CanvasEntityKind::Text {
                            text,
                            text_style,
                            placeholder,
                        } = &entity.kind
                        {
                            let mut edited = text.clone();
                            let response = ui.add_enabled(
                                !entity.locked,
                                egui::TextEdit::multiline(&mut edited)
                                    .id_salt(("canvas-inspector-text", entity.id))
                                    .hint_text(placeholder.as_str())
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(4),
                            );
                            let requested_focus = self.editing_text == Some(entity.id)
                                && std::mem::take(&mut self.focus_text_requested);
                            if requested_focus {
                                response.request_focus();
                            }
                            if response.gained_focus() {
                                self.editing_text = Some(entity.id);
                            }
                            if response.changed() {
                                let mut updated = (*entity).clone();
                                updated.kind = CanvasEntityKind::Text {
                                    text: edited,
                                    text_style: *text_style,
                                    placeholder: placeholder.clone(),
                                };
                                if !text_style.wrap {
                                    updated.transform.size = inspector_text_size(
                                        ui,
                                        text_style,
                                        match &updated.kind {
                                            CanvasEntityKind::Text { text, .. } => text,
                                            _ => unreachable!(),
                                        },
                                    );
                                }
                                self.record_update(vec![(*entity).clone()], vec![updated], true);
                            }
                            let exit = ui.ctx().input(|input| {
                                input.key_pressed(egui::Key::Escape)
                                    || (input.modifiers.command
                                        && input.key_pressed(egui::Key::Enter))
                            });
                            if response.has_focus() && exit {
                                response.surrender_focus();
                            }
                            if response.lost_focus() {
                                if self.editing_text == Some(entity.id) {
                                    self.editing_text = None;
                                }
                                self.block.finish_history_group();
                            }
                        }
                    }

                    let font_size = common_value(texts.iter().map(|style| style.font_size));
                    let mut size = match font_size {
                        CommonValue::Uniform(size) => size,
                        CommonValue::Mixed | CommonValue::None => 18.0,
                    };
                    ui.horizontal(|ui| {
                        ui.label("Font size");
                        if matches!(font_size, CommonValue::Mixed) {
                            ui.weak("Mixed");
                        }
                        if ui
                            .add(
                                egui::DragValue::new(&mut size)
                                    .range(4.0..=256.0)
                                    .speed(1.0)
                                    .suffix(" px"),
                            )
                            .changed()
                        {
                            self.update_selected_text(entities, |style| style.font_size = size);
                        }
                    });

                    let line_height = common_value(texts.iter().map(|style| style.line_height));
                    let mut height = match line_height {
                        CommonValue::Uniform(height) => height,
                        CommonValue::Mixed | CommonValue::None => 1.2,
                    };
                    ui.horizontal(|ui| {
                        ui.label("Line height");
                        if ui
                            .add(
                                egui::DragValue::new(&mut height)
                                    .range(0.5..=4.0)
                                    .speed(0.05),
                            )
                            .changed()
                        {
                            self.update_selected_text(entities, |style| style.line_height = height);
                        }
                    });

                    let bold = common_value(
                        texts
                            .iter()
                            .map(|style| style.weight == CanvasTextWeight::Bold),
                    );
                    if let Some(bold) = mixed_checkbox(ui, "Bold", bold) {
                        self.update_selected_text(entities, |style| {
                            style.weight = if bold {
                                CanvasTextWeight::Bold
                            } else {
                                CanvasTextWeight::Regular
                            };
                        });
                    }

                    let alignment = common_value(texts.iter().map(|style| style.alignment));
                    ui.horizontal(|ui| {
                        ui.label("Alignment");
                        for (label, value) in [
                            ("Left", CanvasTextAlign::Left),
                            ("Center", CanvasTextAlign::Center),
                            ("Right", CanvasTextAlign::Right),
                        ] {
                            if ui
                                .selectable_label(alignment == CommonValue::Uniform(value), label)
                                .clicked()
                            {
                                self.update_selected_text(entities, |style| {
                                    style.alignment = value
                                });
                            }
                        }
                    });

                    let wrap = common_value(texts.iter().map(|style| style.wrap));
                    if let Some(wrap) = mixed_checkbox(ui, "Wrap text", wrap) {
                        self.update_selected_text(entities, |style| style.wrap = wrap);
                    }
                    ui.weak("Resize to wrap; hold Alt while resizing to scale text.");
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
            });

        let mut movement = None;
        egui::CollapsingHeader::new("Arrange")
            .default_open(true)
            .show(ui, |ui| {
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
                ui.horizontal(|ui| {
                    for (label, layer_move) in [
                        ("Back", CanvasLayerMove::SendToBack),
                        ("-1", CanvasLayerMove::BackOne),
                        ("+1", CanvasLayerMove::ForwardOne),
                        ("Front", CanvasLayerMove::BringToFront),
                    ] {
                        if ui
                            .add_enabled(
                                self.can_reorder(entities, layer_move),
                                egui::Button::new(label).small(),
                            )
                            .clicked()
                        {
                            movement = Some(layer_move);
                        }
                    }
                });
                let can_group = self.selection_can_group(entities);
                let can_ungroup = selected.iter().any(|entity| entity.group_id.is_some());
                ui.columns(2, |columns| {
                    if columns[0]
                        .add_enabled(can_group, egui::Button::new("Group"))
                        .clicked()
                    {
                        self.execute_command(CanvasCommand::Group, entities);
                    }
                    if columns[1]
                        .add_enabled(can_ungroup, egui::Button::new("Ungroup"))
                        .clicked()
                    {
                        self.execute_command(CanvasCommand::Ungroup, entities);
                    }
                });
                let can_lock = selected.iter().any(|entity| !entity.locked);
                let can_unlock = selected.iter().any(|entity| entity.locked);
                ui.columns(2, |columns| {
                    if columns[0]
                        .add_enabled(can_lock, egui::Button::new("Lock"))
                        .clicked()
                    {
                        self.execute_command(CanvasCommand::Lock, entities);
                    }
                    if columns[1]
                        .add_enabled(can_unlock, egui::Button::new("Unlock"))
                        .clicked()
                    {
                        self.execute_command(CanvasCommand::Unlock, entities);
                    }
                });
                let can_delete = selected.iter().any(|entity| !entity.locked);
                if ui
                    .add_enabled(can_delete, egui::Button::new("Delete"))
                    .clicked()
                {
                    self.execute_command(CanvasCommand::Delete, entities);
                }
            });
        (movement, editor_action)
    }

    pub(super) fn show_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        entities: &[CanvasEntity],
        _editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) {
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
                }
            }
            if ui.button(ICON_DATA_OBJECT).on_hover_text("Block").clicked() {
                self.pending_block_center = Some(self.viewport_center);
                self.picker.open([self.block.id()]);
            }

            ui.menu_button("Actions", |ui| {
                let has_selection = !self.selection.is_empty();
                for (label, enabled, command) in [
                    ("Cut", has_selection, CanvasCommand::Cut),
                    ("Copy", has_selection, CanvasCommand::Copy),
                    ("Paste", true, CanvasCommand::Paste),
                    ("Duplicate", has_selection, CanvasCommand::Duplicate),
                    ("Delete", has_selection, CanvasCommand::Delete),
                ] {
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        self.execute_command(command, entities);
                        ui.close();
                    }
                }
                ui.separator();
                let selected = entities
                    .iter()
                    .filter(|entity| self.selection.contains(&entity.id))
                    .collect::<Vec<_>>();
                for (label, enabled, command) in [
                    (
                        "Group",
                        self.selection_can_group(entities),
                        CanvasCommand::Group,
                    ),
                    (
                        "Ungroup",
                        selected.iter().any(|entity| entity.group_id.is_some()),
                        CanvasCommand::Ungroup,
                    ),
                    (
                        "Lock",
                        selected.iter().any(|entity| !entity.locked),
                        CanvasCommand::Lock,
                    ),
                    (
                        "Unlock",
                        selected.iter().any(|entity| entity.locked),
                        CanvasCommand::Unlock,
                    ),
                ] {
                    if ui.add_enabled(enabled, egui::Button::new(label)).clicked() {
                        self.execute_command(command, entities);
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
                if ui
                    .add_enabled(has_selection, egui::Button::new("Fit selection"))
                    .clicked()
                {
                    self.fit_selection_requested = true;
                    ui.close();
                }
            })
            .response
            .on_hover_text("Selection and clipboard actions");

            ui.separator();
            if ui
                .small_button(ICON_ZOOM_OUT)
                .on_hover_text("Zoom out")
                .clicked()
            {
                viewport.change_zoom(1.0 / ZOOM_STEP, None);
            }
            if ui
                .small_button(format!("{:.0}%", viewport.zoom() * 100.0))
                .on_hover_text("Reset zoom to 100%")
                .clicked()
            {
                viewport.change_zoom(1.0 / viewport.zoom(), None);
            }
            ui.menu_button(ICON_KEYBOARD_ARROW_DOWN, |ui| {
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
                        self.block
                            .read()
                            .is_some_and(|canvas| canvas.preview_region().is_some()),
                        egui::Button::new("Fit preview region"),
                    )
                    .clicked()
                {
                    self.fit_preview_region_requested = true;
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
        });
        ui.separator();
    }
}

pub(super) fn attach_component(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: BlockRef,
) {
    for entity in entities {
        if selected.contains(&entity.id)
            && !entity
                .components
                .iter()
                .any(|component| component.schema_id == schema_id)
        {
            entity.components.push(CanvasComponent {
                schema_id,
                values: std::collections::BTreeMap::new(),
            });
        }
    }
}

pub(super) fn set_component_value(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: BlockRef,
    field_id: Uuid,
    value: Option<DatabaseValue>,
) {
    for entity in entities {
        if !selected.contains(&entity.id) {
            continue;
        }
        let Some(component) = entity
            .components
            .iter_mut()
            .find(|component| component.schema_id == schema_id)
        else {
            continue;
        };
        if let Some(value) = value.clone() {
            component.values.insert(field_id, value);
        } else {
            component.values.remove(&field_id);
        }
    }
}

pub(super) fn remove_component(
    entities: &mut [CanvasEntity],
    selected: &HashSet<Uuid>,
    schema_id: BlockRef,
) {
    for entity in entities {
        if selected.contains(&entity.id) {
            entity
                .components
                .retain(|component| component.schema_id != schema_id);
        }
    }
}
