//! The Files pane: the tree of blocks in the workspace, the recently deleted
//! list beneath it, and the status line the workspace and account menus hang
//! off.

use std::collections::HashSet;

use block::{Block, BlockAccess, BlockParent, BlockReference, BlockReferenceList};
use block_client::blocks::settings::Settings;
use eframe::egui;
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD, ICON_CHECK, ICON_CIRCLE,
    ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT,
};
use uuid::Uuid;

use crate::{
    access_mode_icon, block_context_menu,
    editors::{SidebarDragPayload, SidebarDragSource},
    performance, BlockApp, BlockContextMenuAction, BlockMenuPermissions, BlockPickerTarget,
    PendingDestructiveAction, RenameState, ICON_DYNAMIC_ARTIFACT, NO_EDIT_ACCESS,
};

struct SidebarActiveLocation {
    id: Uuid,
    label: crate::editors::BlockLabel,
    root: BlockParent,
    path: Vec<Uuid>,
}

struct SidebarHighlight {
    rect: egui::Rect,
    collapsed: bool,
}

const MAX_OPENED_VIA_HOPS: usize = 64;

#[derive(Default)]
struct SidebarRenderState {
    highlight: Option<SidebarHighlight>,
    scroll_requested: bool,
}

impl BlockApp {
    fn open_settings(&mut self) {
        let Some(id) = self
            .root_settings
            .ensure(&self.client)
            .map(|settings| settings.id())
        else {
            return;
        };
        self.open_tab(id, Settings::TYPE_ID);
    }

    fn root_via_roots_list(&self, id: Uuid) -> Option<BlockParent> {
        if !self.roots.is_loaded() {
            return None;
        }
        Some(if self.roots.read().iter().any(|root| root.id == id) {
            BlockParent::Root
        } else {
            BlockParent::Orphaned
        })
    }

    fn sidebar_active_location(&mut self) -> Option<SidebarActiveLocation> {
        let id = self.active_tab?;
        let editor = self.editors.get(&id)?;

        let mut hops = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(id);
        let mut current = id;
        while let Some(&container) = self.opened_via.get(&current) {
            if hops.len() >= MAX_OPENED_VIA_HOPS || !visited.insert(container) {
                break;
            }
            hops.push(container);
            current = container;
        }

        self.parents
            .entry(current)
            .or_insert_with(|| self.client.watch_parents(current));
        let parents = self.parents.get(&current)?;
        if !parents.is_loaded() {
            return None;
        }
        let parents = parents.read();
        let root = if let Some(parent) = parents.first() {
            parent.parent
        } else if current == id {
            match editor.relationships() {
                Some(relationships) => relationships.parent,
                None => self.root_via_roots_list(current)?,
            }
        } else {
            self.root_via_roots_list(current)?
        };
        if matches!(root, BlockParent::Uuid(_)) {
            return None;
        }
        let mut path = parents.iter().map(|parent| parent.id).collect::<Vec<_>>();
        path.extend(hops.iter().rev().copied());
        path.push(id);
        Some(SidebarActiveLocation {
            id,
            label: self.client.cached_block(id).map_or_else(
                || crate::editors::BlockLabel::for_handle(&self.registry, editor.block()),
                |block| crate::editors::BlockLabel::for_cached(&self.registry, &block),
            ),
            root,
            path,
        })
    }

    fn reveal_sidebar_location(&mut self, location: &SidebarActiveLocation) {
        if location.root == BlockParent::Orphaned && !self.orphaned_expanded {
            self.orphaned_expanded = true;
            self.orphaned = Some(self.client.watch_references(BlockReferenceList::Orphans));
        }
        for parent in location
            .path
            .iter()
            .take(location.path.len().saturating_sub(1))
        {
            self.expanded.entry(*parent).or_insert_with(|| {
                self.client
                    .watch_references(BlockReferenceList::References(*parent))
            });
        }
        self.sidebar_reveal = Some(location.id);
    }

    fn reference_label(&self, ui: &egui::Ui, reference: &BlockReference) -> egui::WidgetText {
        crate::editors::BlockLabel::for_reference(&self.registry, reference).widget_text(ui.style())
    }

    /// Whether the account may change a block. Blocks the sidebar has listed
    /// but never opened count as editable until the server says otherwise.
    pub(crate) fn can_edit_block(&self, id: Uuid) -> bool {
        self.client.block_access(id).can_edit()
    }

    /// Whether a block may be taken out of where it is listed. Blocks holding
    /// their children have to accept the removal and be editable; the root and
    /// the orphan list are the workspace's own and hold nothing back.
    fn can_delete_from(&self, source: SidebarDragSource) -> bool {
        match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(id) => {
                self.block_types
                    .get(&id)
                    .is_some_and(|block_type| self.registry.can_delete_child(*block_type))
                    && self.can_edit_block(id)
            }
        }
    }

    /// Whether a block may be moved out of `source`. Moving one that is listed
    /// where it lives reparents the block itself; moving a reference to it only
    /// touches the block that holds the reference.
    pub(crate) fn can_move_out_of(
        &self,
        source: SidebarDragSource,
        child: Uuid,
        is_reference: bool,
    ) -> bool {
        self.can_delete_from(source) && (is_reference || self.can_edit_block(child))
    }

    fn collapse_reference(&mut self, id: Uuid) {
        self.expanded.remove(&id);
    }

    fn show_reference(
        &mut self,
        ui: &mut egui::Ui,
        reference: BlockReference,
        depth: usize,
        containing_id: Option<Uuid>,
        path: &mut HashSet<Uuid>,
        active: Option<&SidebarActiveLocation>,
        active_path_index: Option<usize>,
        sidebar: &mut SidebarRenderState,
    ) {
        self.block_types.insert(reference.id, reference.block_type);
        let is_reference =
            containing_id.is_some_and(|id| reference.parent != BlockParent::Uuid(id));
        let source = containing_id.map_or_else(
            || match reference.parent {
                BlockParent::Orphaned => SidebarDragSource::Orphaned,
                BlockParent::Root | BlockParent::Uuid(_) => SidebarDragSource::Root,
            },
            SidebarDragSource::Block,
        );
        let access = self.client.block_access(reference.id);
        let can_edit = access.can_edit();
        let can_open = access.can_view();
        let can_add_child = self.registry.can_add_child(reference.block_type);
        // Taking a child in means changing the block that takes it.
        let can_add_here = can_add_child && can_edit;
        let can_delete_child = source != SidebarDragSource::Orphaned
            && self.can_move_out_of(source, reference.id, is_reference);
        let copy_permission = self.copy_permission(containing_id);
        let can_expand = !is_reference && reference.references > 0;
        let was_expanded = self.expanded.contains_key(&reference.id);
        // Matched by position, not just id: the same block can appear both at
        // its canonical row and at unrelated reference rows elsewhere, and
        // only the one actually on the route should highlight.
        let on_active_path = active.is_some_and(|active| {
            active_path_index.is_some_and(|index| active.path.get(index) == Some(&reference.id))
        });
        let is_final_active_path_element =
            active.is_some_and(|active| active_path_index == Some(active.path.len() - 1));
        let is_active = on_active_path && is_final_active_path_element;
        let is_closed_active_ancestor =
            !is_reference && !was_expanded && on_active_path && !is_final_active_path_element;
        let mut toggle = false;
        let mut open = false;
        let mut delete = false;
        let mut make_copy = false;
        let mut picker_excluded = path.clone();
        picker_excluded.insert(reference.id);
        let mut row_response = None;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            let expand_response = ui
                .add_enabled(
                    can_expand,
                    egui::Button::selectable(
                        is_closed_active_ancestor,
                        if can_expand {
                            if was_expanded {
                                ICON_KEYBOARD_ARROW_DOWN
                            } else {
                                ICON_KEYBOARD_ARROW_RIGHT
                            }
                        } else if is_reference {
                            ICON_ARROW_FORWARD
                        } else {
                            ICON_CIRCLE
                        },
                    )
                    .small(),
                )
                .on_hover_text(match (can_expand, is_reference) {
                    (false, true) => format!(
                        "{}\nThis block is referenced here, but is not a direct child. It cannot be expanded here.",
                        reference.id
                    ),
                    (false, false) => {
                        format!(
                            "{}\nThis block cannot be expanded because it has no children.",
                            reference.id
                        )
                    }
                    (true, _) => reference.id.to_string(),
                });
            if expand_response.clicked() {
                toggle = true;
            }
            if is_closed_active_ancestor {
                sidebar.highlight = Some(SidebarHighlight {
                    rect: expand_response.rect,
                    collapsed: true,
                });
            }
            let trailing_width = if can_add_child {
                ui.spacing().interact_size.x + ui.spacing().item_spacing.x
            } else {
                0.0
            };
            let label_width = (ui.available_width() - trailing_width).max(0.0);
            let label = egui::Button::selectable(is_active, self.reference_label(ui, &reference))
                .truncate()
                .sense(egui::Sense::click_and_drag());
            // Generated blocks are marked as such, and editable blocks are the
            // common case so only the rest carry the icon for as far as the
            // account may go with them.
            let markers: Vec<_> = [
                reference
                    .dynamic_artifact
                    .then_some(ICON_DYNAMIC_ARTIFACT.codepoint),
                access_mode_icon(access),
            ]
            .into_iter()
            .flatten()
            .collect();
            let label = if markers.is_empty() {
                label.right_text(())
            } else {
                label.right_text(markers.join(" "))
            };
            let response = ui
                .add_enabled_ui(can_open, |ui| {
                    ui.add_sized([label_width, ui.spacing().interact_size.y], label)
                })
                .inner;
            let response = match access {
                BlockAccess::Edit => response,
                BlockAccess::View => response.on_hover_text(format!(
                    "{}\nYou can view this block, but not change it.",
                    reference.id
                )),
                BlockAccess::KnowExists | BlockAccess::None => response.on_disabled_hover_text(
                    format!(
                        "{}\nYou can see that this block exists, but not open it.",
                        reference.id
                    ),
                ),
            };
            let response = if reference.dynamic_artifact {
                response.on_hover_text(format!(
                    "{}\nThis block is generated from another block.",
                    ICON_DYNAMIC_ARTIFACT.codepoint
                ))
            } else {
                response
            };
            // A block that may only be known to exist cannot be opened, but its
            // row still has to answer right clicks and drags: a reference to one
            // could otherwise never be taken out of the block holding it.
            let response = if can_open {
                response
            } else {
                ui.interact(
                    response.rect,
                    response.id.with("locked"),
                    egui::Sense::click_and_drag(),
                )
            };
            response.dnd_set_drag_payload(SidebarDragPayload {
                reference: reference.clone(),
                source,
                is_reference,
            });
            if can_open && response.clicked() {
                open = true;
            }
            if is_active {
                if self.sidebar_reveal == Some(reference.id) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    self.sidebar_reveal = None;
                    sidebar.scroll_requested = true;
                }
                sidebar.highlight = Some(SidebarHighlight {
                    rect: response.rect,
                    collapsed: false,
                });
            }
            response.context_menu(|ui| {
                match block_context_menu(
                    ui,
                    &self.registry,
                    &mut self.block_picker,
                    &self.client,
                    &mut self.parent_candidates,
                    reference.id,
                    reference.parent,
                    picker_excluded.clone(),
                    BlockMenuPermissions {
                        add: can_add_here,
                        edit: can_edit,
                        delete: can_delete_child,
                        copy: copy_permission,
                    },
                    is_reference,
                ) {
                    Some(BlockContextMenuAction::Picker) => {
                        self.block_picker_target = Some(BlockPickerTarget::Block {
                            parent: reference.id,
                            open: true,
                        });
                    }
                    Some(BlockContextMenuAction::SetParent(parent)) => {
                        self.set_block_parent(reference.id, parent);
                    }
                    Some(BlockContextMenuAction::Rename) => {
                        let name =
                            crate::editors::BlockLabel::for_reference(&self.registry, &reference)
                                .name;
                        self.rename = Some(RenameState {
                            id: reference.id,
                            name,
                        });
                    }
                    Some(BlockContextMenuAction::Share) => {
                        let label =
                            crate::editors::BlockLabel::for_reference(&self.registry, &reference);
                        self.share.open(&self.client, reference.id, label);
                    }
                    Some(BlockContextMenuAction::Copy) => make_copy = true,
                    Some(BlockContextMenuAction::Delete) => delete = true,
                    None => {}
                }
            });
            row_response = Some(response);
            if can_add_child {
                ui.add_enabled_ui(can_add_here, |ui| {
                    if ui.button(ICON_ADD).clicked() {
                        self.block_picker_target = Some(BlockPickerTarget::Block {
                            parent: reference.id,
                            open: true,
                        });
                        self.block_picker.open(picker_excluded.clone());
                    }
                })
                .response
                .on_hover_text("Add a child")
                .on_disabled_hover_text(NO_EDIT_ACCESS);
            }
        });

        if delete {
            self.queue_delete(reference.clone(), source, is_reference);
        }
        if make_copy {
            if let Some(container) = containing_id {
                self.queue_copy(reference.id, container, Uuid::new_v4());
            }
        }
        if can_add_child {
            if let Some(response) = row_response {
                if let Some(dragged) = response.dnd_hover_payload::<SidebarDragPayload>() {
                    let valid = can_add_here
                        && dragged.reference.id != reference.id
                        && dragged.source != SidebarDragSource::Block(reference.id)
                        && !path.contains(&dragged.reference.id)
                        && self.can_move_out_of(
                            dragged.source,
                            dragged.reference.id,
                            dragged.is_reference,
                        );
                    let color = if valid {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().error_fg_color
                    };
                    ui.painter().rect_stroke(
                        response.rect,
                        3.0,
                        egui::Stroke::new(1.0_f32, color),
                        egui::StrokeKind::Outside,
                    );
                }
                if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
                    if can_add_here
                        && dragged.reference.id != reference.id
                        && dragged.source != SidebarDragSource::Block(reference.id)
                        && !path.contains(&dragged.reference.id)
                        && self.can_move_out_of(
                            dragged.source,
                            dragged.reference.id,
                            dragged.is_reference,
                        )
                    {
                        self.queue_move(dragged.as_ref().clone(), reference.id);
                    }
                }
            }
        }

        if toggle {
            if was_expanded {
                self.collapse_reference(reference.id);
            } else {
                self.expanded.insert(
                    reference.id,
                    self.client
                        .watch_references(BlockReferenceList::References(reference.id)),
                );
            }
        }
        if open {
            match containing_id {
                Some(container) => {
                    self.opened_via.insert(reference.id, container);
                }
                None => {
                    self.opened_via.remove(&reference.id);
                }
            }
            self.open_tab(reference.id, reference.block_type);
        }

        let child_active_path_index = if on_active_path {
            active_path_index.map(|index| index + 1)
        } else {
            None
        };
        let is_expanded = can_expand && self.expanded.contains_key(&reference.id);
        if !is_expanded || !path.insert(reference.id) {
            return;
        }
        let children = self.expanded[&reference.id].read();
        if children.is_empty() && self.expanded[&reference.id].is_loaded() {
            ui.horizontal(|ui| {
                ui.add_space((depth + 1) as f32 * 14.0 + 22.0);
                ui.weak("No references");
            });
        }
        for child in children {
            self.show_reference(
                ui,
                child,
                depth + 1,
                Some(reference.id),
                path,
                active,
                child_active_path_index,
                sidebar,
            );
        }
        path.remove(&reference.id);
    }

    pub(crate) fn show_sidebar(&mut self, ui: &mut egui::Ui) {
        if self.sidebar_reveal.is_some() && self.sidebar_reveal != self.active_tab {
            self.sidebar_reveal = None;
        }
        let active = self.sidebar_active_location();
        ui.horizontal(|ui| {
            ui.heading("Blocks");
            ui.add_space(ui.available_width() - 28.0);
            if ui.button(ICON_ADD).on_hover_text("Create block").clicked() {
                self.block_picker_target = Some(BlockPickerTarget::Root);
                self.block_picker.open([]);
            }
        });
        ui.separator();

        egui::Panel::bottom("file-list-status")
            .resizable(false)
            .show_inside(ui, |ui| self.show_file_list_status(ui));

        let mut sidebar = SidebarRenderState::default();
        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
            let roots = self.roots.read();
            if roots.is_empty() && self.roots.is_loaded() {
                ui.weak("No root blocks");
            }
            for root in roots {
                self.show_reference(
                    ui,
                    root,
                    0,
                    None,
                    &mut HashSet::new(),
                    active.as_ref(),
                    Some(0),
                    &mut sidebar,
                );
            }
            ui.separator();
            self.show_recently_deleted(ui, active.as_ref(), &mut sidebar);
        });
        if sidebar.scroll_requested {
            return;
        }
        let Some(active) = active else {
            return;
        };
        let Some(highlight) = sidebar.highlight else {
            return;
        };
        let viewport = scroll.inner_rect;
        let at_top = highlight.rect.bottom() < viewport.top();
        let at_bottom = highlight.rect.top() > viewport.bottom();
        let (arrow, y) = if at_top {
            (ICON_ARROW_UPWARD, viewport.top())
        } else if at_bottom {
            (
                egui_material_icons::icons::ICON_ARROW_DOWNWARD,
                viewport.bottom() - ui.spacing().interact_size.y,
            )
        } else if highlight.collapsed {
            (
                ICON_ARROW_UPWARD,
                viewport.bottom() - ui.spacing().interact_size.y,
            )
        } else {
            return;
        };
        let rect = egui::Rect::from_min_size(
            egui::pos2(viewport.left(), y),
            egui::vec2(viewport.width(), ui.spacing().interact_size.y),
        );
        let mut job = egui::text::LayoutJob::default();
        egui::RichText::new(format!("{} (", arrow.codepoint)).append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        active.label.rich_text().append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        egui::RichText::new(")").append_to(
            &mut job,
            ui.style(),
            egui::FontSelection::Style(egui::TextStyle::Button),
            egui::Align::Center,
        );
        if ui.put(rect, egui::Button::new(job)).clicked() {
            self.reveal_sidebar_location(&active);
        }
    }

    fn show_recently_deleted(
        &mut self,
        ui: &mut egui::Ui,
        active: Option<&SidebarActiveLocation>,
        sidebar: &mut SidebarRenderState,
    ) {
        let mut toggle = false;
        ui.horizontal(|ui| {
            let is_closed_active_ancestor = !self.orphaned_expanded
                && active.is_some_and(|active| active.root == BlockParent::Orphaned);
            let expand_response = ui.add(
                egui::Button::selectable(
                    is_closed_active_ancestor,
                    if self.orphaned_expanded {
                        ICON_KEYBOARD_ARROW_DOWN
                    } else {
                        ICON_KEYBOARD_ARROW_RIGHT
                    },
                )
                .small(),
            );
            if expand_response.clicked() {
                toggle = true;
            }
            if is_closed_active_ancestor {
                sidebar.highlight = Some(SidebarHighlight {
                    rect: expand_response.rect,
                    collapsed: true,
                });
            }
            if ui
                .selectable_label(false, "Recently Deleted")
                .on_hover_text("Blocks that no longer have a parent")
                .clicked()
            {
                toggle = true;
            }
        });

        if toggle {
            self.orphaned_expanded = !self.orphaned_expanded;
            self.orphaned = self
                .orphaned_expanded
                .then(|| self.client.watch_references(BlockReferenceList::Orphans));
        }

        if !self.orphaned_expanded {
            return;
        }
        let Some(orphaned) = &self.orphaned else {
            return;
        };
        let blocks = orphaned.read();
        let loaded = orphaned.is_loaded();
        if blocks.is_empty() && loaded {
            ui.horizontal(|ui| {
                ui.add_space(36.0);
                ui.weak("No recently deleted blocks");
            });
        }
        for block in blocks {
            self.show_reference(
                ui,
                block,
                1,
                None,
                &mut HashSet::new(),
                active,
                Some(0),
                sidebar,
            );
        }
    }

    fn show_file_list_status(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        let debug = self.client.network_debug_snapshot();
        ui.horizontal(|ui| {
            if debug.changes_saved {
                ui.horizontal(|ui| {
                    ui.small(ICON_CHECK);
                    ui.small("All changes saved");
                });
            } else {
                ui.spinner();
                ui.small("Submitting changes\u{2026}");
            }
            if let Some((frame, duration)) = performance::last_frame() {
                ui.separator();
                ui.small(format!(
                    "Frame {frame}: {:.3} ms",
                    duration.as_secs_f64() * 1_000.0
                ));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("More", |ui| {
                    if ui.button("Settings").clicked() {
                        self.open_settings();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Client").clicked() {
                        self.client_debug_open = true;
                        ui.close();
                    }
                    if ui.button("Network").clicked() {
                        self.network_debug_open = true;
                        ui.close();
                    }
                    if ui.button("Performance").clicked() {
                        performance::open();
                        ui.close();
                    }
                    if ui.button("Plugins").clicked() {
                        crate::debug::plugins::open();
                        ui.close();
                    }
                    if ui.button("Version").clicked() {
                        crate::debug::version::open();
                        ui.close();
                    }
                    #[cfg(not(any(
                        target_os = "android",
                        target_os = "windows",
                        target_os = "macos",
                        target_arch = "wasm32"
                    )))]
                    if ui.button("Terminal").clicked() {
                        crate::debug::terminal::open();
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Workspace");
                    if let Some(workspace) = &self.workspace {
                        ui.small(&workspace.name);
                    }
                    if ui.button("Invite member").clicked() {
                        self.invite_open = true;
                        ui.close();
                    }
                    if ui.button("Switch workspace").clicked() {
                        if debug.changes_saved {
                            self.scheduled_workspace_list = true;
                        } else {
                            self.pending_destructive_action =
                                Some(PendingDestructiveAction::ChooseWorkspace);
                        }
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Accounts");
                    ui.small(format!("Signed in as {}", self.account.name));
                    ui.small(self.account.id.to_string());
                    for account in self.accounts.clone() {
                        if ui
                            .selectable_label(account == self.account, &account.name)
                            .on_hover_text(account.id.to_string())
                            .clicked()
                        {
                            self.request_account_switch(account);
                            ui.close();
                        }
                    }
                    if ui
                        .add_enabled(debug.changes_saved, egui::Button::new("Manage accounts"))
                        .on_disabled_hover_text("Wait for changes to finish saving")
                        .clicked()
                    {
                        self.signed_in = false;
                        if let Err(error) = self.app_state.clear_active_account() {
                            self.account_error = Some(error.to_string());
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("About").clicked() {
                        self.about_open = true;
                        ui.close();
                    }
                });
            });
        });
    }
}
