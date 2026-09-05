use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use block::{BlockAccess, BlockParent, BlockReference, BlockReferenceList};
use block_client::{blocks, BlockClient, BlockHandleAccess, ReferenceList};
use block_editor_plugin::{
    block_ui::{BlockCatalog, BlockLabel, BlockTypes},
    egui,
    egui_material_icons::icons::{
        ICON_ADD, ICON_ARROW_DOWNWARD, ICON_ARROW_FORWARD, ICON_ARROW_UPWARD, ICON_AUTO_AWESOME,
        ICON_CIRCLE, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT, ICON_LINK_OFF, ICON_LOCK,
        ICON_SHARE, ICON_VISIBILITY,
    },
    BlockFilter, BlockPicker, BlockSource, EditorHost,
};
use uuid::Uuid;

const NO_EDIT_ACCESS: &str = "You do not have permission to change this block";
const INDENT: f32 = 14.0;

struct Tree {
    host: EditorHost,
    client: Arc<BlockClient>,
    block_id: Uuid,
    roots: ReferenceList,
}

#[derive(Clone, Copy)]
enum PickerTarget {
    Root,
    Child { parent: Uuid, open: bool },
}

#[derive(Clone)]
struct DragPayload {
    reference: BlockReference,
    source: BlockSource,
    is_reference: bool,
}

struct ActiveLocation {
    id: Uuid,
    label: BlockLabel,
    root: BlockParent,
    path: Vec<Uuid>,
}

struct Highlight {
    rect: egui::Rect,
    collapsed: bool,
}

#[derive(Default)]
struct RenderState {
    highlight: Option<Highlight>,
    scroll_requested: bool,
}

enum ContextAction {
    Picker,
    SetParent(BlockParent),
    Rename,
    Share,
    Unlink,
    Delete,
}

#[derive(Default)]
pub struct FileTreeApp {
    tree: Option<Tree>,
    orphaned: Option<ReferenceList>,
    orphaned_expanded: bool,
    expanded: HashMap<Uuid, ReferenceList>,
    parents: Option<(Uuid, ReferenceList)>,
    focused: Option<(Uuid, Box<dyn BlockHandleAccess>)>,
    parent_candidates: HashMap<Uuid, ReferenceList>,
    block_types: HashMap<Uuid, Uuid>,
    reveal: Option<Uuid>,
    picker: BlockPicker,
    picker_target: Option<PickerTarget>,
    picker_error: Option<String>,
}

struct Frame<'a> {
    host: &'a EditorHost,
    client: &'a BlockClient,
    types: &'a BlockCatalog,
}

impl Frame<'_> {
    fn can_edit(&self, id: Uuid) -> bool {
        self.client.block_access(id).can_edit()
    }
}

impl FileTreeApp {
    fn open_picker(&mut self, target: PickerTarget, excluded: impl IntoIterator<Item = Uuid>) {
        let Some(tree) = &self.tree else {
            return;
        };
        self.picker_error = None;
        self.picker_target = Some(target);
        self.picker.open(
            &tree.host,
            BlockFilter {
                name: "Block".to_owned(),
                block_types: Vec::new(),
                excluded: excluded.into_iter().map(Uuid::into_bytes).collect(),
                templates: false,
            },
        );
    }

    fn poll_picker(&mut self) {
        let Some(tree) = &self.tree else {
            return;
        };
        let Some(result) = self.picker.poll(&tree.host) else {
            return;
        };
        let target = self.picker_target.take();
        let picked = match result {
            Ok(picked) => picked,
            Err(error) => {
                self.picker_error = Some(error);
                return;
            }
        };
        self.block_types.insert(picked.id, picked.block_type);
        match target {
            None | Some(PickerTarget::Root) => {
                if !picked.linked {
                    tree.client.set_block_parent(picked.id, BlockParent::Root);
                }
                tree.host.open_block(picked.id, picked.block_type);
            }
            Some(PickerTarget::Child { parent, open }) => {
                tree.host
                    .place_block(picked.id, picked.block_type, parent, picked.linked);
                if open {
                    tree.host
                        .open_block_via(picked.id, picked.block_type, parent);
                }
            }
        }
    }

    fn root_via_roots_list(&self, id: Uuid) -> Option<BlockParent> {
        let tree = self.tree.as_ref()?;
        if !tree.roots.is_loaded() {
            return None;
        }
        Some(if tree.roots.read().iter().any(|root| root.id == id) {
            BlockParent::Root
        } else {
            BlockParent::Orphaned
        })
    }

    fn active_location(&mut self, frame: &Frame<'_>) -> Option<ActiveLocation> {
        let focused = frame.host.focused_block();
        let id = focused.block_id?;
        let current = focused.via.last().copied().unwrap_or(id);
        if self.parents.as_ref().map(|(watched, _)| *watched) != Some(current) {
            self.parents = Some((current, frame.client.watch_parents(current)));
        }
        let (_, parents) = self.parents.as_ref()?;
        if !parents.is_loaded() {
            return None;
        }
        let parents = parents.read();
        let root = if let Some(parent) = parents.first() {
            parent.parent
        } else if current == id {
            match self.focused_relationship(frame, id, focused.block_type) {
                Some(parent) => parent,
                None => self.root_via_roots_list(current)?,
            }
        } else {
            self.root_via_roots_list(current)?
        };
        if matches!(root, BlockParent::Uuid(_)) {
            return None;
        }
        let mut path: Vec<_> = parents.iter().map(|parent| parent.id).collect();
        path.extend(focused.via.iter().rev().copied());
        path.push(id);
        Some(ActiveLocation {
            id,
            label: frame.client.cached_block(id).map_or_else(
                || BlockLabel::new(frame.types, focused.block_type, None),
                |cached| BlockLabel::for_cached(frame.types, &cached),
            ),
            root,
            path,
        })
    }

    fn focused_relationship(
        &mut self,
        frame: &Frame<'_>,
        id: Uuid,
        block_type: Uuid,
    ) -> Option<BlockParent> {
        if self.focused.as_ref().map(|(open, _)| *open) != Some(id) {
            self.focused = blocks::open(frame.client, id, block_type).map(|block| (id, block));
        }
        let (_, block) = self.focused.as_ref()?;
        Some(block.relationships()?.parent)
    }

    fn reveal_location(&mut self, frame: &Frame<'_>, location: &ActiveLocation) {
        if location.root == BlockParent::Orphaned && !self.orphaned_expanded {
            self.orphaned_expanded = true;
            self.orphaned = Some(frame.client.watch_references(BlockReferenceList::Orphans));
        }
        for parent in location
            .path
            .iter()
            .take(location.path.len().saturating_sub(1))
        {
            self.expanded.entry(*parent).or_insert_with(|| {
                frame
                    .client
                    .watch_references(BlockReferenceList::References(*parent))
            });
        }
        self.reveal = Some(location.id);
    }

    fn can_delete_from(&self, frame: &Frame<'_>, source: BlockSource) -> bool {
        match source {
            BlockSource::Root | BlockSource::Orphaned => true,
            BlockSource::Block(id) => {
                self.block_types
                    .get(&id)
                    .is_some_and(|block_type| frame.types.child_edits(*block_type).delete)
                    && frame.can_edit(id)
            }
        }
    }

    fn can_move_out_of(
        &self,
        frame: &Frame<'_>,
        source: BlockSource,
        child: Uuid,
        is_reference: bool,
    ) -> bool {
        self.can_delete_from(frame, source) && (is_reference || frame.can_edit(child))
    }

    fn unlink_permission(
        &self,
        frame: &Frame<'_>,
        container: Option<Uuid>,
    ) -> Result<(), &'static str> {
        let container_type = container.and_then(|id| self.block_types.get(&id).copied());
        match (container, container_type) {
            (Some(_), Some(block_type)) if !frame.types.child_edits(block_type).replace => {
                Err("This container doesn't support replacing a reference")
            }
            (Some(container), Some(_)) if !frame.can_edit(container) => {
                Err("You don't have permission to edit this container")
            }
            (Some(_), Some(_)) => Ok(()),
            _ => Err("Loading\u{2026}"),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn show_reference(
        &mut self,
        ui: &mut egui::Ui,
        frame: &Frame<'_>,
        reference: BlockReference,
        depth: usize,
        containing_id: Option<Uuid>,
        path: &mut HashSet<Uuid>,
        active: Option<&ActiveLocation>,
        active_path_index: Option<usize>,
        state: &mut RenderState,
    ) {
        self.block_types.insert(reference.id, reference.block_type);
        let is_reference =
            containing_id.is_some_and(|id| reference.parent != BlockParent::Uuid(id));
        let source = containing_id.map_or_else(
            || match reference.parent {
                BlockParent::Orphaned => BlockSource::Orphaned,
                BlockParent::Root | BlockParent::Uuid(_) => BlockSource::Root,
            },
            BlockSource::Block,
        );
        let access = frame.client.block_access(reference.id);
        let can_edit = access.can_edit();
        let can_open = access.can_view();
        let can_add_child = frame.types.child_edits(reference.block_type).add;

        let can_add_here = can_add_child && can_edit;
        let can_delete_child = source != BlockSource::Orphaned
            && self.can_move_out_of(frame, source, reference.id, is_reference);
        let unlink_permission = self.unlink_permission(frame, containing_id);
        let can_expand = !is_reference && reference.references > 0;
        let was_expanded = self.expanded.contains_key(&reference.id);

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
        let mut action = None;
        let mut picker_excluded = path.clone();
        picker_excluded.insert(reference.id);
        let mut row_response = None;
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * INDENT);
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
                    (false, false) => format!(
                        "{}\nThis block cannot be expanded because it has no children.",
                        reference.id
                    ),
                    (true, _) => reference.id.to_string(),
                });
            if expand_response.clicked() {
                toggle = true;
            }
            if is_closed_active_ancestor {
                state.highlight = Some(Highlight {
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
            let label = egui::Button::selectable(
                is_active,
                BlockLabel::for_reference(frame.types, &reference).widget_text(ui.style()),
            )
            .truncate()
            .sense(egui::Sense::click_and_drag());
            let markers: Vec<_> = [
                reference
                    .dynamic_artifact
                    .then_some(ICON_AUTO_AWESOME.codepoint),
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
                    ICON_AUTO_AWESOME.codepoint
                ))
            } else {
                response
            };
            let response = if can_open {
                response
            } else {
                ui.interact(
                    response.rect,
                    response.id.with("locked"),
                    egui::Sense::click_and_drag(),
                )
            };
            if response.drag_started() {
                frame
                    .host
                    .drag_block(reference.id, reference.block_type);
            }
            response.dnd_set_drag_payload(DragPayload {
                reference: reference.clone(),
                source,
                is_reference,
            });
            if can_open && response.clicked() {
                open = true;
            }
            if is_active {
                if self.reveal == Some(reference.id) {
                    response.scroll_to_me(Some(egui::Align::Center));
                    self.reveal = None;
                    state.scroll_requested = true;
                }
                state.highlight = Some(Highlight {
                    rect: response.rect,
                    collapsed: false,
                });
            }
            response.context_menu(|ui| {
                action = context_menu(
                    ui,
                    frame,
                    &mut self.parent_candidates,
                    reference.id,
                    reference.parent,
                    Permissions {
                        add: can_add_here,
                        edit: can_edit,
                        delete: can_delete_child,
                        unlink: unlink_permission,
                    },
                    is_reference,
                );
            });
            row_response = Some(response);
            if can_add_child {
                ui.add_enabled_ui(can_add_here, |ui| {
                    if ui.button(ICON_ADD).clicked() {
                        action = Some(ContextAction::Picker);
                    }
                })
                .response
                .on_hover_text("Add a child")
                .on_disabled_hover_text(NO_EDIT_ACCESS);
            }
        });

        match action {
            Some(ContextAction::Picker) => {
                self.open_picker(
                    PickerTarget::Child {
                        parent: reference.id,
                        open: true,
                    },
                    picker_excluded,
                );
            }
            Some(ContextAction::SetParent(parent)) => {
                frame.client.set_block_parent(reference.id, parent);
            }
            Some(ContextAction::Rename) => frame.host.rename_block(reference.id),
            Some(ContextAction::Share) => frame.host.share_block(reference.id),
            Some(ContextAction::Unlink) => {
                if let Some(container) = containing_id {
                    frame.host.unlink_block(reference.id, container);
                }
            }
            Some(ContextAction::Delete) => {
                frame
                    .host
                    .delete_block(reference.id, reference.block_type, source, is_reference)
            }
            None => {}
        }

        if can_add_child {
            if let Some(response) = row_response {
                let accepts = |dragged: &DragPayload| {
                    can_add_here
                        && dragged.reference.id != reference.id
                        && dragged.source != BlockSource::Block(reference.id)
                        && !path.contains(&dragged.reference.id)
                        && self.can_move_out_of(
                            frame,
                            dragged.source,
                            dragged.reference.id,
                            dragged.is_reference,
                        )
                };
                if let Some(dragged) = response.dnd_hover_payload::<DragPayload>() {
                    let color = if accepts(&dragged) {
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
                if let Some(dragged) = response.dnd_release_payload::<DragPayload>() {
                    if accepts(&dragged) {
                        frame.host.move_block(
                            dragged.reference.id,
                            dragged.reference.block_type,
                            dragged.source,
                            reference.id,
                            dragged.is_reference,
                        );
                    }
                }
            }
        }

        if toggle {
            if was_expanded {
                self.expanded.remove(&reference.id);
            } else {
                self.expanded.insert(
                    reference.id,
                    frame
                        .client
                        .watch_references(BlockReferenceList::References(reference.id)),
                );
            }
        }
        if open {
            match containing_id {
                Some(container) => {
                    frame
                        .host
                        .open_block_via(reference.id, reference.block_type, container);
                }
                None => frame.host.open_block(reference.id, reference.block_type),
            }
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
                ui.add_space((depth + 1) as f32 * INDENT + 22.0);
                ui.weak("No references");
            });
        }
        for child in children {
            self.show_reference(
                ui,
                frame,
                child,
                depth + 1,
                Some(reference.id),
                path,
                active,
                child_active_path_index,
                state,
            );
        }
        path.remove(&reference.id);
    }

    fn show_recently_deleted(
        &mut self,
        ui: &mut egui::Ui,
        frame: &Frame<'_>,
        active: Option<&ActiveLocation>,
        state: &mut RenderState,
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
                state.highlight = Some(Highlight {
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
                .then(|| frame.client.watch_references(BlockReferenceList::Orphans));
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
                frame,
                block,
                1,
                None,
                &mut HashSet::new(),
                active,
                Some(0),
                state,
            );
        }
    }
}

impl block_editor_plugin::App for FileTreeApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        let roots = client.watch_references(BlockReferenceList::Roots);
        self.tree = Some(Tree {
            host,
            client,
            block_id,
            roots,
        });
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(block_id) = self.tree.as_ref().map(|tree| tree.block_id) else {
            return;
        };
        if ui.button(ICON_ADD).on_hover_text("Create block").clicked() {
            self.open_picker(PickerTarget::Root, [block_id]);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_picker();
        let Some((host, client)) = self
            .tree
            .as_ref()
            .map(|tree| (tree.host.clone(), Arc::clone(&tree.client)))
        else {
            return;
        };
        let types = host.block_types();
        let frame = Frame {
            host: &host,
            client: &client,
            types: types.as_ref(),
        };

        let active = self.active_location(&frame);
        if self.reveal.is_some() && self.reveal != active.as_ref().map(|active| active.id) {
            self.reveal = None;
        }
        if let Some(error) = self.picker_error.clone() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }

        let mut state = RenderState::default();
        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            let roots = self
                .tree
                .as_ref()
                .map(|tree| (tree.roots.read(), tree.roots.is_loaded()));
            let Some((roots, loaded)) = roots else {
                return;
            };
            if roots.is_empty() && loaded {
                ui.weak("No root blocks");
            }
            for root in roots {
                self.show_reference(
                    ui,
                    &frame,
                    root,
                    0,
                    None,
                    &mut HashSet::new(),
                    active.as_ref(),
                    Some(0),
                    &mut state,
                );
            }
            ui.separator();
            self.show_recently_deleted(ui, &frame, active.as_ref(), &mut state);
        });
        if state.scroll_requested {
            return;
        }
        let (Some(active), Some(highlight)) = (active, state.highlight) else {
            return;
        };
        let viewport = scroll.inner_rect;
        let at_top = highlight.rect.bottom() < viewport.top();
        let at_bottom = highlight.rect.top() > viewport.bottom();
        let (arrow, y) = if at_top {
            (ICON_ARROW_UPWARD, viewport.top())
        } else if at_bottom {
            (
                ICON_ARROW_DOWNWARD,
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
            self.reveal_location(&frame, &active);
        }
    }
}

struct Permissions {
    add: bool,
    edit: bool,
    delete: bool,
    unlink: Result<(), &'static str>,
}

fn context_menu(
    ui: &mut egui::Ui,
    frame: &Frame<'_>,
    parent_candidates: &mut HashMap<Uuid, ReferenceList>,
    subject: Uuid,
    current_parent: BlockParent,
    permissions: Permissions,
    is_reference: bool,
) -> Option<ContextAction> {
    let mut action = None;
    ui.add_enabled_ui(permissions.add, |ui| {
        if ui.button("Add").clicked() {
            action = Some(ContextAction::Picker);
            ui.close();
        }
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    ui.add_enabled_ui(permissions.edit, |ui| {
        ui.menu_button("Set parent", |ui| {
            if ui
                .add_enabled(
                    current_parent != BlockParent::Root,
                    egui::Button::new("Root"),
                )
                .clicked()
            {
                action = Some(ContextAction::SetParent(BlockParent::Root));
                ui.close();
            }
            if ui
                .add_enabled(
                    current_parent != BlockParent::Orphaned,
                    egui::Button::new("Orphaned"),
                )
                .clicked()
            {
                action = Some(ContextAction::SetParent(BlockParent::Orphaned));
                ui.close();
            }
            ui.separator();
            let backrefs = parent_candidates.entry(subject).or_insert_with(|| {
                frame
                    .client
                    .watch_references(BlockReferenceList::Backrefs(subject))
            });
            let listed = backrefs.read();
            if listed.is_empty() {
                ui.weak(if backrefs.is_loaded() {
                    "No backrefs"
                } else {
                    "Loading\u{2026}"
                });
            }
            for backref in listed {
                let is_current = current_parent == BlockParent::Uuid(backref.id);
                let label =
                    BlockLabel::for_reference(frame.types, &backref).widget_text(ui.style());
                if ui
                    .add_enabled(!is_current, egui::Button::new(label))
                    .clicked()
                {
                    action = Some(ContextAction::SetParent(BlockParent::Uuid(backref.id)));
                    ui.close();
                }
            }
        });
    })
    .response
    .on_disabled_hover_text(NO_EDIT_ACCESS);
    if ui
        .add_enabled(permissions.edit, egui::Button::new("Rename"))
        .on_disabled_hover_text(NO_EDIT_ACCESS)
        .clicked()
    {
        action = Some(ContextAction::Rename);
        ui.close();
    }
    if ui
        .add_enabled(
            permissions.edit,
            egui::Button::new(format!("{} Share", ICON_SHARE.codepoint)),
        )
        .on_disabled_hover_text("Only accounts that can edit a block may share it")
        .clicked()
    {
        action = Some(ContextAction::Share);
        ui.close();
    }
    if is_reference {
        let unlink_button = ui.add_enabled(
            permissions.unlink.is_ok(),
            egui::Button::new(format!("{} Unlink", ICON_LINK_OFF.codepoint)),
        );
        let clicked = match permissions.unlink {
            Ok(()) => unlink_button
                .on_hover_text(
                    "Replace this occurrence with its own copy, unaffected by the original",
                )
                .clicked(),
            Err(hover) => unlink_button.on_disabled_hover_text(hover).clicked(),
        };
        if clicked {
            action = Some(ContextAction::Unlink);
            ui.close();
        }
    }
    let delete_label = if is_reference {
        "Remove link"
    } else {
        "Delete"
    };
    let delete_text = egui::RichText::new(delete_label);
    let delete_text = if permissions.delete {
        delete_text.color(ui.visuals().error_fg_color)
    } else {
        delete_text
    };
    let delete_response = ui.add_enabled(permissions.delete, egui::Button::new(delete_text));
    let delete_response = if is_reference {
        delete_response.on_hover_text(
            "Removes this link only, without creating a copy. The original block is not deleted.",
        )
    } else {
        delete_response
    };
    if delete_response.clicked() {
        action = Some(ContextAction::Delete);
        ui.close();
    }
    action
}

fn access_mode_icon(access: BlockAccess) -> Option<&'static str> {
    match access {
        BlockAccess::Edit => None,
        BlockAccess::View => Some(ICON_VISIBILITY.codepoint),
        BlockAccess::KnowExists | BlockAccess::None => Some(ICON_LOCK.codepoint),
    }
}

#[cfg(test)]
mod tests;
