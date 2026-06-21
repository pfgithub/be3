use super::*;

pub(super) fn default_hotbar() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Wire),
        HotbarSlot::Builtin(ToolKind::MergerSplitter),
        HotbarSlot::Folder {
            name: "Component".to_string(),
            slots: default_component_slots(),
        },
        HotbarSlot::Folder {
            name: "Logic".to_string(),
            slots: vec![HotbarSlot::Builtin(ToolKind::Not)],
        },
        HotbarSlot::Folder {
            name: "Storage".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Storage),
                HotbarSlot::Builtin(ToolKind::ConfigureStorage),
                HotbarSlot::Locked {
                    name: "Register".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Memory".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Display".to_string(),
            slots: vec![
                HotbarSlot::Builtin(ToolKind::Led),
                HotbarSlot::Locked {
                    name: "Seven Segment".to_string(),
                },
            ],
        },
        HotbarSlot::Folder {
            name: "Organization".to_string(),
            slots: vec![
                HotbarSlot::Locked {
                    name: "Comment".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Pattern".to_string(),
                },
                HotbarSlot::Locked {
                    name: "Group".to_string(),
                },
            ],
        },
    ]
}

pub(super) fn default_component_slots() -> Vec<HotbarSlot> {
    vec![
        HotbarSlot::Builtin(ToolKind::Input),
        HotbarSlot::Builtin(ToolKind::Output),
    ]
}

impl LogicEditor {
    pub fn set_component_files(&mut self, component_files: Option<ComponentFiles>) {
        self.component_files = component_files;
    }

    pub(super) fn select_tool(&mut self) {
        self.tool.kind = ToolKind::Select;
        self.active_hotbar_slot = None;
        self.gesture = None;
        self.configured_storage = None;
    }

    pub(super) fn select_hotbar_path(&mut self, path: Vec<usize>) {
        let Some(slot) = get_hotbar_slot(&self.hotbar, &path) else {
            return;
        };
        match slot {
            HotbarSlot::Builtin(kind) => {
                self.tool.kind = *kind;
                self.active_hotbar_slot = Some(path);
                self.gesture = None;
                self.configured_storage = None;
                self.selection.clear();
            }
            HotbarSlot::Custom { .. } => {
                self.select_custom_hotbar_path(path);
            }
            HotbarSlot::Folder { .. } => {
                self.active_hotbar_folder = path;
            }
            HotbarSlot::Locked { .. } => {}
        }
    }

    fn select_custom_hotbar_path(&mut self, path: Vec<usize>) {
        self.tool.kind = ToolKind::Custom;
        self.active_hotbar_slot = Some(path);
        self.gesture = None;
        self.configured_storage = None;
        self.selection.clear();
    }

    /// Rebuilds the hotbar from persisted entries. An empty save keeps the
    /// default tree so new saves do not need to write default data eagerly.
    pub fn set_hotbar(&mut self, slots: Vec<SaveHotbarSlot>) {
        self.hotbar = if slots.is_empty() {
            default_hotbar()
        } else {
            slots
                .into_iter()
                .filter_map(hotbar_slot_from_save)
                .collect()
        };
        if self.tool.kind == ToolKind::Custom {
            self.tool.kind = ToolKind::Select;
            self.active_hotbar_slot = None;
        }
        self.active_hotbar_folder.clear();
    }

    /// Appends a compiled custom component to the hotbar. If a slot for the same
    /// source already exists it is updated in place instead of duplicated.
    pub fn add_custom_hotbar_slot(
        &mut self,
        name: String,
        source: ComponentFileRef,
        kind: ComponentKind,
    ) {
        if let Some(slot) = find_custom_hotbar_slot_mut(&mut self.hotbar, source) {
            *slot = HotbarSlot::Custom { name, source, kind };
        } else {
            self.hotbar.push(HotbarSlot::Custom { name, source, kind });
        }
        self.persist_hotbar();
    }

    /// Unpins a custom hotbar slot, persisting the change and fixing up the
    /// selected custom path when needed.
    pub(super) fn remove_hotbar_slot(&mut self, path: &[usize]) {
        let Some(HotbarSlot::Custom { source, .. }) = get_hotbar_slot(&self.hotbar, path) else {
            return;
        };
        let source = *source;
        if let Some(files) = &self.component_files {
            if let Err(error) = files.remove_hotbar(source) {
                eprintln!("failed to unpin hotbar component: {error}");
            }
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.active_hotbar_slot = None;
            if self.tool.kind == ToolKind::Custom {
                self.select_tool();
            }
        }
    }

    pub(super) fn remove_hotbar_folder(&mut self, path: &[usize]) {
        if !matches!(
            get_hotbar_slot(&self.hotbar, path),
            Some(HotbarSlot::Folder { .. })
        ) || hotbar_slot_contains_unremovable(get_hotbar_slot(&self.hotbar, path).unwrap())
        {
            return;
        }
        remove_hotbar_slot_at(&mut self.hotbar, path);
        self.persist_hotbar();
        if self
            .active_hotbar_slot
            .as_ref()
            .is_some_and(|active| active.starts_with(path))
        {
            self.select_tool();
        }
        if self.active_hotbar_folder.starts_with(path) {
            self.active_hotbar_folder.clear();
        }
    }

    pub(super) fn reset_hotbar(&mut self) {
        self.hotbar = default_hotbar();
        self.active_hotbar_folder.clear();
        self.select_tool();
        self.persist_hotbar();
    }

    pub(super) fn persist_hotbar(&self) {
        let Some(files) = &self.component_files else {
            return;
        };
        if let Err(error) = files.save_hotbar(self.hotbar.iter().map(hotbar_slot_to_save).collect())
        {
            eprintln!("failed to save hotbar: {error}");
        }
    }

    pub(super) fn show_hotbar(&mut self, ui: &mut egui::Ui) {
        let mut action: Option<HotbarAction> = None;
        let mut remove: Option<Vec<usize>> = None;
        let mut remove_folder: Option<Vec<usize>> = None;
        let mut new_folder = false;
        let mut drop_target: Option<HotbarDropTarget> = None;
        let pointer_released = ui.ctx().input(|input| input.pointer.any_released());
        let dragging_hotbar = self.hotbar_drag.is_some();
        let dragged_slot = self
            .hotbar_drag
            .as_ref()
            .and_then(|path| get_hotbar_slot(&self.hotbar, path))
            .cloned();
        let rows = visible_hotbar_rows(&self.hotbar, &self.active_hotbar_folder);
        let key_entries = rows
            .iter()
            .rev()
            .find(|row| !row.entries.is_empty())
            .map(|row| row.entries.as_slice())
            .unwrap_or(&[]);

        if !ui.ctx().egui_wants_keyboard_input() {
            ui.ctx().input(|input| {
                for (index, key) in HOTBAR_KEYS.iter().enumerate() {
                    if input.key_pressed(*key) {
                        if let Some((path, _)) = key_entries.get(index) {
                            action = Some(HotbarAction::SelectPath(path.clone()));
                        }
                    }
                }
                if input.key_pressed(egui::Key::Backtick) {
                    self.active_hotbar_folder.pop();
                }
            });
        }

        let current_folder = self.active_hotbar_folder.clone();
        let hotbar_header = ui.horizontal(|ui| {
            let up_response = ui.add_enabled(
                !self.active_hotbar_folder.is_empty(),
                egui::Button::new("Up"),
            );
            if up_response.clicked() {
                self.active_hotbar_folder.pop();
            }
            if up_response.hovered() && pointer_released {
                let mut parent = current_folder.clone();
                parent.pop();
                drop_target = Some(HotbarDropTarget::Folder(parent));
            }
            if !self.active_hotbar_folder.is_empty() {
                ui.small(hotbar_folder_name(&self.hotbar, &self.active_hotbar_folder));
            } else {
                ui.small("Hotbar");
            }
        });
        if hotbar_header.response.hovered() && pointer_released {
            drop_target = Some(HotbarDropTarget::Folder(current_folder));
        }
        if dragging_hotbar && hotbar_header.response.hovered() {
            ui.painter().rect_stroke(
                hotbar_header.response.rect.expand(2.0),
                4.0,
                egui::Stroke::new(1.5, ui.visuals().selection.stroke.color),
                egui::StrokeKind::Inside,
            );
        }

        ui.menu_button("+", |ui| {
            if ui.button("New folder").clicked() {
                new_folder = true;
                ui.close();
            }
            if ui.button("Reset hotbar").clicked() {
                self.confirm_hotbar_reset = true;
                ui.close();
            }
        });

        let visible_row_count = if self.active_hotbar_folder.is_empty() {
            1
        } else {
            rows.len()
        };
        let column_height = ui.available_height();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = HOTBAR_COLUMN_GAP;
            for (row_index, row) in rows.iter().enumerate().take(visible_row_count) {
                let mut hovered_hotbar_slot = false;
                let row_response = egui::Frame::group(ui.style())
                    .fill(if row.active {
                        ui.visuals().selection.bg_fill.gamma_multiply(0.28)
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    })
                    .show(ui, |ui| {
                        let column_width = HOTBAR_COLUMN_WIDTH - 16.0;
                        ui.set_width(column_width);
                        ui.set_min_height(column_height);
                        ui.vertical_centered(|ui| {
                            let (label_rect, _) = ui.allocate_exact_size(
                                egui::vec2(column_width, 18.0),
                                egui::Sense::hover(),
                            );
                            ui.painter().text(
                                label_rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &row.title,
                                egui::FontId::proportional(11.0),
                                ui.visuals().weak_text_color(),
                            );
                            egui::ScrollArea::vertical()
                                .id_salt(("hotbar-row", row_index, row.folder_path.as_slice()))
                                .auto_shrink([false, false])
                                .max_height((column_height - 24.0).max(HOTBAR_SLOT_SIZE))
                                .show(ui, |ui| {
                                    ui.set_width(column_width);
                                    ui.vertical_centered(|ui| {
                                        if row.entries.is_empty() {
                                            ui.allocate_exact_size(
                                                egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
                                                egui::Sense::hover(),
                                            );
                                        }
                                        for (path, slot) in &row.entries {
                                            let hotkey = key_entries
                                                .iter()
                                                .position(|(key_path, _)| key_path == path)
                                                .and_then(hotbar_key_label);
                                            let selected =
                                                self.active_hotbar_slot.as_ref() == Some(path);
                                            let open_folder = self.active_hotbar_folder == *path;
                                            let dragging = self.hotbar_drag.as_ref() == Some(path);
                                            let drop_highlight =
                                                self.hotbar_drag.as_ref().is_some_and(|source| {
                                                    source != path
                                                        && !path.starts_with(source)
                                                        && hotbar_slot_drop_target(
                                                            &self.hotbar,
                                                            path,
                                                        )
                                                        .is_some()
                                                });
                                            let label = self.hotbar_slot_label(slot);
                                            let tooltip = self.hotbar_slot_tooltip(slot, &label);
                                            let preview_slot = self.hotbar_display_slot(slot);
                                            let response = hotbar_button(
                                                ui,
                                                selected,
                                                open_folder,
                                                dragging,
                                                drop_highlight,
                                                &label,
                                                &tooltip,
                                                hotkey,
                                                |painter, rect| {
                                                    paint_slot_preview(
                                                        painter,
                                                        rect,
                                                        &preview_slot,
                                                    );
                                                },
                                            );
                                            if response.clicked() {
                                                action =
                                                    Some(HotbarAction::SelectPath(path.clone()));
                                            }
                                            if response.drag_started() {
                                                self.hotbar_drag = Some(path.clone());
                                            }
                                            if response.hovered() {
                                                hovered_hotbar_slot = true;
                                                if pointer_released {
                                                    drop_target =
                                                        hotbar_slot_drop_target(&self.hotbar, path);
                                                }
                                            }
                                            if matches!(
                                                slot,
                                                HotbarSlot::Custom { .. }
                                                    | HotbarSlot::Folder { .. }
                                            ) {
                                                response.context_menu(|ui| {
                                                    if matches!(slot, HotbarSlot::Custom { .. })
                                                        && ui.button("Remove from hotbar").clicked()
                                                    {
                                                        remove = Some(path.clone());
                                                        ui.close();
                                                    }
                                                    if matches!(slot, HotbarSlot::Folder { .. })
                                                        && ui.button("Open folder").clicked()
                                                    {
                                                        action = Some(HotbarAction::OpenFolder(
                                                            path.clone(),
                                                        ));
                                                        ui.close();
                                                    }
                                                    if matches!(slot, HotbarSlot::Folder { .. })
                                                        && !hotbar_slot_contains_unremovable(slot)
                                                        && ui.button("Remove folder").clicked()
                                                    {
                                                        remove_folder = Some(path.clone());
                                                        ui.close();
                                                    }
                                                });
                                            }
                                        }
                                    });
                                });
                        });
                    });
                if row_response.response.hovered() && pointer_released && !hovered_hotbar_slot {
                    drop_target = Some(HotbarDropTarget::Folder(row.folder_path.clone()));
                }
            }
        });

        if let Some(path) = remove {
            self.remove_hotbar_slot(&path);
        }
        if let Some(path) = remove_folder {
            self.remove_hotbar_folder(&path);
        }
        if new_folder {
            let slots = get_hotbar_slots_mut(&mut self.hotbar, &self.active_hotbar_folder);
            slots.push(HotbarSlot::Folder {
                name: "Folder".to_string(),
                slots: Vec::new(),
            });
            self.persist_hotbar();
        }
        if pointer_released {
            if let (Some(source), Some(target)) = (self.hotbar_drag.take(), drop_target) {
                match target {
                    HotbarDropTarget::Slot(target) => {
                        move_hotbar_slot(&mut self.hotbar, &source, &target);
                    }
                    HotbarDropTarget::Folder(target) => {
                        move_hotbar_slot_to_folder(&mut self.hotbar, &source, &target);
                    }
                }
                self.persist_hotbar();
                if self
                    .active_hotbar_slot
                    .as_ref()
                    .is_some_and(|active| active.starts_with(&source))
                {
                    self.select_tool();
                }
                if self.active_hotbar_folder.starts_with(&source) {
                    self.active_hotbar_folder.clear();
                }
            } else {
                self.hotbar_drag = None;
            }
        } else if let Some(slot) = dragged_slot.as_ref() {
            paint_hotbar_drag_preview(ui.ctx(), slot);
        }

        match action {
            Some(HotbarAction::SelectPath(path)) => {
                if self.active_hotbar_slot.as_ref() == Some(&path) {
                    self.select_tool();
                } else {
                    self.select_hotbar_path(path);
                }
            }
            Some(HotbarAction::OpenFolder(path)) => {
                if self.active_hotbar_folder == path {
                    self.active_hotbar_folder.pop();
                } else {
                    self.active_hotbar_folder = path;
                }
            }
            None => {}
        }

        self.show_hotbar_reset_confirmation(ui.ctx());
    }

    pub(super) fn show_tool_settings(&mut self, ui: &mut egui::Ui) {
        // Scale controls. They have no effect on custom components for now.
        if matches!(self.tool.kind, ToolKind::MergerSplitter) {
            ui.small("Input scale");
            scale_buttons(ui, &mut self.tool.scale);
            ui.small("Output scale");
            scale_buttons(ui, &mut self.tool.merger_out_scale);
        } else {
            ui.small("Scale");
            scale_buttons(ui, &mut self.tool.scale);
        }

        // A label for freely placed inputs/outputs. Inside a challenge the label
        // is fixed by the assigned port, so the field is not offered.
        if matches!(self.tool.kind, ToolKind::Input | ToolKind::Output) && self.challenge.is_none()
        {
            ui.separator();
            ui.small("Label");
            ui.add(egui::TextEdit::singleline(&mut self.io_label).desired_width(f32::INFINITY));
        }

        ui.separator();
        ui.small("Middle drag: pan");
        ui.small("Wheel: zoom");
        ui.small("Shift: add/remove");
        ui.small("Delete: remove");
        ui.small("Esc: cancel");
    }

    pub(super) fn show_hotbar_reset_confirmation(&mut self, context: &egui::Context) {
        if !self.confirm_hotbar_reset {
            return;
        }

        let mut reset = false;
        let mut cancel = false;
        let response = egui::Modal::new("reset-hotbar-modal".into()).show(context, |ui| {
            ui.set_min_width(300.0);
            ui.heading("Reset hotbar?");
            ui.label("This will restore the default hotbar layout.");
            ui.horizontal(|ui| {
                reset = ui.button("Reset").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if response.should_close() || cancel {
            self.confirm_hotbar_reset = false;
        }
        if reset {
            self.confirm_hotbar_reset = false;
            self.reset_hotbar();
        }
    }
}
pub(super) enum HotbarAction {
    SelectPath(Vec<usize>),
    OpenFolder(Vec<usize>),
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum HotbarDropTarget {
    Slot(Vec<usize>),
    Folder(Vec<usize>),
}

pub(super) fn hotbar_key_label(index: usize) -> Option<&'static str> {
    match index {
        0 => Some("1"),
        1 => Some("2"),
        2 => Some("3"),
        3 => Some("4"),
        4 => Some("5"),
        5 => Some("6"),
        6 => Some("7"),
        7 => Some("8"),
        8 => Some("9"),
        9 => Some("0"),
        _ => None,
    }
}

pub(super) fn hotbar_slot_from_save(slot: SaveHotbarSlot) -> Option<HotbarSlot> {
    Some(match slot {
        SaveHotbarSlot::Builtin { tool } => HotbarSlot::Builtin(ToolKind::from_id(&tool)?),
        SaveHotbarSlot::Locked { name } => HotbarSlot::Locked { name },
        SaveHotbarSlot::Folder { name, slots } => HotbarSlot::Folder {
            name,
            slots: slots
                .into_iter()
                .filter_map(hotbar_slot_from_save)
                .collect(),
        },
        SaveHotbarSlot::Custom { name, kind } => HotbarSlot::Custom {
            name,
            source: hotbar_kind_source(&kind)?,
            kind,
        },
    })
}

pub(super) fn hotbar_slot_to_save(slot: &HotbarSlot) -> SaveHotbarSlot {
    match slot {
        HotbarSlot::Builtin(kind) => SaveHotbarSlot::Builtin {
            tool: kind.id().to_string(),
        },
        HotbarSlot::Locked { name } => SaveHotbarSlot::Locked { name: name.clone() },
        HotbarSlot::Folder { name, slots } => SaveHotbarSlot::Folder {
            name: name.clone(),
            slots: slots.iter().map(hotbar_slot_to_save).collect(),
        },
        HotbarSlot::Custom { name, kind, .. } => SaveHotbarSlot::Custom {
            name: name.clone(),
            kind: kind.clone(),
        },
    }
}

pub(super) fn hotbar_kind_source(kind: &ComponentKind) -> Option<ComponentFileRef> {
    match kind {
        ComponentKind::Subcomponent { source_file_id, .. } => Some(*source_file_id),
        _ => None,
    }
}

pub(super) fn hotbar_slot_contains_unremovable(slot: &HotbarSlot) -> bool {
    match slot {
        HotbarSlot::Builtin(_) | HotbarSlot::Locked { .. } => true,
        HotbarSlot::Folder { slots, .. } => slots.iter().any(hotbar_slot_contains_unremovable),
        HotbarSlot::Custom { .. } => false,
    }
}

pub(super) struct HotbarRow {
    pub(super) folder_path: Vec<usize>,
    pub(super) title: String,
    pub(super) active: bool,
    pub(super) entries: Vec<(Vec<usize>, HotbarSlot)>,
}

pub(super) fn visible_hotbar_rows(slots: &[HotbarSlot], active_folder: &[usize]) -> [HotbarRow; 2] {
    let (first_path, second_path) = if active_folder.len() <= 1 {
        (
            Vec::new(),
            (!active_folder.is_empty()).then(|| active_folder.to_vec()),
        )
    } else {
        (
            active_folder[..active_folder.len() - 1].to_vec(),
            Some(active_folder.to_vec()),
        )
    };
    let first_active = second_path.is_none();
    [
        hotbar_row(slots, first_path, first_active),
        match second_path {
            Some(path) => hotbar_row(slots, path, true),
            None => HotbarRow {
                folder_path: Vec::new(),
                title: "Open folder".to_string(),
                active: false,
                entries: Vec::new(),
            },
        },
    ]
}

pub(super) fn hotbar_row(slots: &[HotbarSlot], folder_path: Vec<usize>, active: bool) -> HotbarRow {
    let title = hotbar_folder_name(slots, &folder_path).to_string();
    let entries = visible_hotbar_entries(slots, &folder_path);
    HotbarRow {
        folder_path,
        title,
        active,
        entries,
    }
}

pub(super) fn visible_hotbar_entries(
    slots: &[HotbarSlot],
    folder_path: &[usize],
) -> Vec<(Vec<usize>, HotbarSlot)> {
    let slots = get_hotbar_slots(slots, folder_path).unwrap_or(slots);
    slots
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, slot)| {
            let mut path = folder_path.to_vec();
            path.push(index);
            (path, slot)
        })
        .collect()
}

pub(super) fn hotbar_folder_name<'a>(slots: &'a [HotbarSlot], folder_path: &[usize]) -> &'a str {
    match get_hotbar_slot(slots, folder_path) {
        Some(HotbarSlot::Folder { name, .. }) => name,
        _ => "Hotbar",
    }
}

pub(super) fn get_hotbar_slots<'a>(
    slots: &'a [HotbarSlot],
    folder_path: &[usize],
) -> Option<&'a [HotbarSlot]> {
    match folder_path.split_first() {
        None => Some(slots),
        Some((index, rest)) => match slots.get(*index)? {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots(slots, rest),
            _ => None,
        },
    }
}

pub(super) fn get_hotbar_slots_mut<'a>(
    slots: &'a mut Vec<HotbarSlot>,
    folder_path: &[usize],
) -> &'a mut Vec<HotbarSlot> {
    match folder_path.split_first() {
        None => slots,
        Some((index, rest)) => match slots
            .get_mut(*index)
            .expect("active hotbar folder path is valid")
        {
            HotbarSlot::Folder { slots, .. } => get_hotbar_slots_mut(slots, rest),
            _ => panic!("active hotbar folder path points to a non-folder"),
        },
    }
}

pub(super) fn get_hotbar_slot<'a>(
    slots: &'a [HotbarSlot],
    path: &[usize],
) -> Option<&'a HotbarSlot> {
    let (index, rest) = path.split_first()?;
    let slot = slots.get(*index)?;
    if rest.is_empty() {
        return Some(slot);
    }
    match slot {
        HotbarSlot::Folder { slots, .. } => get_hotbar_slot(slots, rest),
        _ => None,
    }
}

pub(super) fn find_custom_hotbar_slot_mut(
    slots: &mut [HotbarSlot],
    source: ComponentFileRef,
) -> Option<&mut HotbarSlot> {
    for slot in slots {
        match slot {
            HotbarSlot::Custom {
                source: existing, ..
            } if *existing == source => return Some(slot),
            HotbarSlot::Folder { slots, .. } => {
                if let Some(found) = find_custom_hotbar_slot_mut(slots, source) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn remove_hotbar_slot_at(
    slots: &mut Vec<HotbarSlot>,
    path: &[usize],
) -> Option<HotbarSlot> {
    let (index, parent_path) = path.split_last()?;
    let parent = get_hotbar_slots_mut(slots, parent_path);
    (*index < parent.len()).then(|| parent.remove(*index))
}

pub(super) fn hotbar_slot_drop_target(
    slots: &[HotbarSlot],
    path: &[usize],
) -> Option<HotbarDropTarget> {
    if matches!(
        get_hotbar_slot(slots, path),
        Some(HotbarSlot::Folder { .. })
    ) {
        None
    } else {
        Some(HotbarDropTarget::Slot(path.to_vec()))
    }
}

pub(super) fn move_hotbar_slot_to_folder(
    slots: &mut Vec<HotbarSlot>,
    source: &[usize],
    folder_path: &[usize],
) {
    if source == folder_path || folder_path.starts_with(source) {
        return;
    }
    let Some(slot) = remove_hotbar_slot_at(slots, source) else {
        return;
    };

    let mut adjusted_folder = folder_path.to_vec();
    if !folder_path.is_empty()
        && source.len() == folder_path.len()
        && source[..source.len() - 1] == folder_path[..folder_path.len() - 1]
        && source[source.len() - 1] < folder_path[folder_path.len() - 1]
    {
        *adjusted_folder
            .last_mut()
            .expect("folder path is non-empty") -= 1;
    }

    get_hotbar_slots_mut(slots, &adjusted_folder).push(slot);
}

pub(super) fn move_hotbar_slot(slots: &mut Vec<HotbarSlot>, source: &[usize], target: &[usize]) {
    if source == target || target.starts_with(source) {
        return;
    }
    let Some(slot) = remove_hotbar_slot_at(slots, source) else {
        return;
    };

    if target.is_empty() {
        slots.push(slot);
        return;
    }

    let mut adjusted_target = target.to_vec();
    if source.len() == target.len()
        && source[..source.len() - 1] == target[..target.len() - 1]
        && source[source.len() - 1] < target[target.len() - 1]
    {
        *adjusted_target
            .last_mut()
            .expect("target path is non-empty") -= 1;
    }

    let Some((index, parent_path)) = adjusted_target.split_last() else {
        return;
    };
    let parent = get_hotbar_slots_mut(slots, parent_path);
    parent.insert((*index).min(parent.len()), slot);
}

pub(super) fn color32(color: [f32; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        (color[0] * 255.0).round() as u8,
        (color[1] * 255.0).round() as u8,
        (color[2] * 255.0).round() as u8,
        (color[3] * 255.0).round() as u8,
    )
}

pub(super) fn scale_buttons(ui: &mut egui::Ui, scale: &mut Scale) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for value in SCALES {
            let candidate = Scale::new(value).expect("hotbar scale is valid");
            if ui
                .selectable_label(*scale == candidate, format!("{value}x"))
                .clicked()
            {
                *scale = candidate;
            }
        }
    });
}

/// A square hotbar slot: a framed button with a glyph preview painted in its top
/// portion and a label across the bottom. Returns the click response.
pub(super) fn hotbar_button(
    ui: &mut egui::Ui,
    selected: bool,
    open_folder: bool,
    dragging: bool,
    drop_target: bool,
    label: &str,
    tooltip: &str,
    hotkey: Option<&str>,
    paint_preview: impl FnOnce(&egui::Painter, egui::Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
        egui::Sense::click_and_drag(),
    );
    let visuals = ui.visuals();
    let (bg, stroke) = if dragging {
        (
            visuals.widgets.inactive.bg_fill.gamma_multiply(0.65),
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if drop_target && response.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            egui::Stroke::new(2.0, visuals.selection.stroke.color),
        )
    } else if selected {
        (
            visuals.selection.bg_fill,
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if open_folder {
        (
            visuals.widgets.active.bg_fill,
            egui::Stroke::new(1.5, visuals.selection.stroke.color),
        )
    } else if response.hovered() {
        (
            visuals.widgets.hovered.bg_fill,
            egui::Stroke::new(1.0, visuals.widgets.hovered.bg_stroke.color),
        )
    } else {
        (
            visuals.widgets.inactive.bg_fill,
            egui::Stroke::new(1.0, visuals.widgets.inactive.bg_stroke.color),
        )
    };
    let text_color = if dragging {
        visuals.weak_text_color()
    } else {
        visuals.text_color()
    };
    let painter = ui.painter();
    painter.rect(rect, 4.0, bg, stroke, egui::StrokeKind::Inside);
    let inner = rect.shrink(4.0);
    let preview_rect =
        egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), inner.height() * 0.66));
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(inner.min.x, preview_rect.max.y),
        egui::pos2(inner.max.x, inner.max.y),
    );
    paint_preview(painter, preview_rect);
    if let Some(hotkey) = hotkey {
        let badge_rect =
            egui::Rect::from_min_size(rect.min + egui::vec2(4.0, 4.0), egui::vec2(15.0, 15.0));
        painter.rect_filled(badge_rect, 3.0, visuals.extreme_bg_color);
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            hotkey,
            egui::FontId::proportional(9.0),
            text_color,
        );
    }
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(9.0),
        text_color,
    );
    response.on_hover_text(tooltip)
}

pub(super) fn paint_hotbar_drag_preview(context: &egui::Context, slot: &HotbarSlot) {
    let Some(pointer) = context.pointer_hover_pos() else {
        return;
    };
    let rect = egui::Rect::from_min_size(
        pointer + egui::vec2(14.0, 14.0),
        egui::vec2(HOTBAR_SLOT_SIZE, HOTBAR_SLOT_SIZE),
    );
    let painter = context.debug_painter();
    painter.rect(
        rect,
        4.0,
        egui::Color32::from_rgba_unmultiplied(24, 28, 36, 224),
        egui::Stroke::new(1.5, egui::Color32::from_rgb(140, 190, 255)),
        egui::StrokeKind::Inside,
    );
    let inner = rect.shrink(4.0);
    let preview_rect =
        egui::Rect::from_min_size(inner.min, egui::vec2(inner.width(), inner.height() * 0.66));
    let label_rect = egui::Rect::from_min_max(
        egui::pos2(inner.min.x, preview_rect.max.y),
        egui::pos2(inner.max.x, inner.max.y),
    );
    paint_slot_preview(&painter, preview_rect, slot);
    painter.text(
        label_rect.center(),
        egui::Align2::CENTER_CENTER,
        slot.label(),
        egui::FontId::proportional(9.0),
        egui::Color32::from_rgb(232, 236, 245),
    );
}

pub(super) fn glyph_point(rect: egui::Rect, x: f32, y: f32) -> egui::Pos2 {
    egui::pos2(
        rect.min.x + x * rect.width(),
        rect.min.y + y * rect.height(),
    )
}

pub(super) fn glyph_inset(rect: egui::Rect, x0: f32, y0: f32, x1: f32, y1: f32) -> egui::Rect {
    egui::Rect::from_min_max(glyph_point(rect, x0, y0), glyph_point(rect, x1, y1))
}

pub(super) fn paint_glyph_box(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_stroke(
        glyph_inset(rect, 0.08, 0.08, 0.92, 0.92),
        0.0,
        egui::Stroke::new(1.5, color),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn paint_glyph_triangle(
    painter: &egui::Painter,
    rect: egui::Rect,
    points: [[f32; 2]; 3],
    color: egui::Color32,
) {
    painter.add(egui::Shape::convex_polygon(
        points
            .iter()
            .map(|point| glyph_point(rect, point[0], point[1]))
            .collect(),
        color,
        egui::Stroke::NONE,
    ));
}

pub(super) fn paint_glyph_diamond(
    painter: &egui::Painter,
    rect: egui::Rect,
    cx: f32,
    cy: f32,
    radius: f32,
    color: egui::Color32,
) {
    let center = glyph_point(rect, cx, cy);
    let rx = radius * rect.width();
    let ry = radius * rect.height();
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(center.x, center.y - ry),
            egui::pos2(center.x + rx, center.y),
            egui::pos2(center.x, center.y + ry),
            egui::pos2(center.x - rx, center.y),
        ],
        color,
        egui::Stroke::NONE,
    ));
}

pub(super) fn paint_port_glyph(painter: &egui::Painter, rect: egui::Rect, kind: ToolKind) {
    let accent = match kind {
        ToolKind::Output => color32(DrawTriangle::OUTPUT_COLOR),
        _ => color32(DrawTriangle::INPUT_COLOR),
    };
    paint_glyph_box(painter, rect, color32(DrawTriangle::GATE_COLOR));
    paint_glyph_triangle(
        painter,
        rect,
        [[0.28, 0.52], [0.72, 0.52], [0.5, 0.2]],
        accent,
    );
    painter.rect_filled(glyph_inset(rect, 0.42, 0.52, 0.58, 0.82), 0.0, accent);
}

pub(super) fn paint_slot_preview(painter: &egui::Painter, rect: egui::Rect, slot: &HotbarSlot) {
    let gate = color32(DrawTriangle::GATE_COLOR);
    match slot {
        HotbarSlot::Builtin(ToolKind::Select) => {
            paint_glyph_triangle(painter, rect, [[0.3, 0.15], [0.3, 0.78], [0.46, 0.6]], gate);
            paint_glyph_triangle(
                painter,
                rect,
                [[0.3, 0.15], [0.46, 0.6], [0.62, 0.55]],
                gate,
            );
        }
        HotbarSlot::Builtin(ToolKind::Wire) => {
            painter.line_segment(
                [glyph_point(rect, 0.15, 0.85), glyph_point(rect, 0.85, 0.15)],
                egui::Stroke::new(3.0, color32(DrawTriangle::WIRE_COLOR)),
            );
        }
        HotbarSlot::Builtin(ToolKind::Not) => {
            paint_glyph_box(painter, rect, gate);
            paint_glyph_triangle(
                painter,
                rect,
                [[0.12, 0.82], [0.88, 0.82], [0.5, 0.12]],
                gate,
            );
        }
        HotbarSlot::Builtin(ToolKind::MergerSplitter) => {
            paint_glyph_box(painter, rect, gate);
            for triangle in [
                [[0.12, 0.82], [0.88, 0.82], [0.62, 0.5]],
                [[0.12, 0.18], [0.62, 0.5], [0.88, 0.18]],
            ] {
                paint_glyph_triangle(painter, rect, triangle, gate);
            }
        }
        HotbarSlot::Builtin(ToolKind::Led) => {
            paint_glyph_box(painter, rect, gate);
            paint_glyph_diamond(painter, rect, 0.5, 0.5, 0.3, gate);
        }
        HotbarSlot::Builtin(ToolKind::Storage) => {
            paint_glyph_box(painter, rect, gate);
            painter.rect_stroke(
                glyph_inset(rect, 0.22, 0.18, 0.78, 0.82),
                0.0,
                egui::Stroke::new(1.5, gate),
                egui::StrokeKind::Inside,
            );
        }
        HotbarSlot::Builtin(ToolKind::ConfigureStorage) => {
            paint_glyph_box(painter, rect, gate);
            painter.rect_stroke(
                glyph_inset(rect, 0.22, 0.18, 0.78, 0.82),
                0.0,
                egui::Stroke::new(1.5, gate),
                egui::StrokeKind::Inside,
            );
            paint_glyph_diamond(
                painter,
                rect,
                0.5,
                0.5,
                0.12,
                color32(DrawTriangle::HIGHLIGHT_COLOR),
            );
        }
        HotbarSlot::Builtin(ToolKind::Input) => paint_port_glyph(painter, rect, ToolKind::Input),
        HotbarSlot::Builtin(ToolKind::Output) => paint_port_glyph(painter, rect, ToolKind::Output),
        HotbarSlot::Builtin(ToolKind::Custom) => paint_glyph_box(painter, rect, gate),
        HotbarSlot::Locked { .. } => {
            paint_glyph_box(painter, rect, egui::Color32::DARK_GRAY);
            let lock_rect = glyph_inset(rect, 0.32, 0.42, 0.68, 0.78);
            painter.rect_filled(lock_rect, 1.5, egui::Color32::DARK_GRAY);
            painter.circle_stroke(
                glyph_point(rect, 0.5, 0.42),
                rect.width() * 0.13,
                egui::Stroke::new(2.0, egui::Color32::DARK_GRAY),
            );
        }
        HotbarSlot::Folder { .. } => {
            let folder = color32(DrawTriangle::HIGHLIGHT_COLOR);
            painter.rect_filled(glyph_inset(rect, 0.12, 0.3, 0.88, 0.78), 2.0, folder);
            painter.rect_filled(glyph_inset(rect, 0.18, 0.2, 0.52, 0.38), 2.0, folder);
        }
        HotbarSlot::Custom { .. } => {
            paint_glyph_box(painter, rect, gate);
            for (x, y) in [(0.5, 0.08), (0.5, 0.92)] {
                painter.circle_filled(
                    glyph_point(rect, x, y),
                    2.5,
                    color32(DrawTriangle::WIRE_COLOR),
                );
            }
        }
    }
}

impl LogicEditor {
    pub(super) fn hotbar_display_slot(&self, slot: &HotbarSlot) -> HotbarSlot {
        slot.clone()
    }

    pub(super) fn hotbar_slot_label(&self, slot: &HotbarSlot) -> String {
        slot.label().to_owned()
    }

    pub(super) fn hotbar_slot_tooltip(&self, _slot: &HotbarSlot, label: &str) -> String {
        label.to_owned()
    }

    pub(super) fn selected_hotbar_kind(&self, path: &[usize]) -> Option<ComponentKind> {
        get_hotbar_slot(&self.hotbar, path).and_then(|slot| match slot {
            HotbarSlot::Custom { kind, .. } => Some(kind.clone()),
            _ => None,
        })
    }
}
