use std::collections::{BTreeMap, HashSet};

use block::BlockParent;
use block_client::{
    properties::{self, BlockName},
    BlockClient, CachedBlock,
};
use eframe::egui;
use uuid::Uuid;

use crate::editors::{
    cached_display_name, BlockCreation, BlockEditor, EditorAccess, EditorRegistry, PendingCreation,
};

enum BlockPickerMenuAction {
    New(Uuid),
    LinkExisting,
}

/// A block whose creation is waiting on options the user has not filled in.
struct PendingBlock {
    block_type: Uuid,
    creation: Box<dyn PendingCreation>,
}

pub struct BlockPickerResult {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Uuid,
    pub properties: BTreeMap<Uuid, Vec<u8>>,
}

pub struct BlockPicker {
    id: Uuid,
    open: bool,
    search: String,
    excluded: HashSet<Uuid>,
    pending_action: Option<BlockPickerMenuAction>,
    pending_block: Option<PendingBlock>,
    error: Option<String>,
}

impl Default for BlockPicker {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            open: false,
            search: String::new(),
            excluded: HashSet::new(),
            pending_action: None,
            pending_block: None,
            error: None,
        }
    }
}

impl BlockPicker {
    pub fn show_menu(&mut self, ui: &mut egui::Ui, registry: &EditorRegistry) {
        self.show_menu_excluding(ui, registry, []);
    }

    pub fn show_menu_excluding(
        &mut self,
        ui: &mut egui::Ui,
        registry: &EditorRegistry,
        excluded: impl IntoIterator<Item = Uuid>,
    ) {
        let mut action = None;
        ui.menu_button("New block", |ui| {
            for &(label, block_type) in registry.new_block_actions() {
                if ui.button(label).clicked() {
                    action = Some(BlockPickerMenuAction::New(block_type));
                    ui.close();
                }
            }
        });
        if ui.button("Link existing block").clicked() {
            action = Some(BlockPickerMenuAction::LinkExisting);
            ui.close();
        }
        if let Some(action) = action {
            self.pending_action = Some(action);
            self.excluded = excluded.into_iter().collect();
        }
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn show_link_picker(
        &mut self,
        context: &egui::Context,
        client: &BlockClient,
        registry: &EditorRegistry,
    ) -> Option<CachedBlock> {
        if !self.open {
            return None;
        }

        let search = self.search.trim().to_lowercase();
        let blocks: Vec<_> = client
            .cached_blocks()
            .into_iter()
            .filter(|block| !self.excluded.contains(&block.id))
            .filter(|block| {
                search.is_empty()
                    || cached_display_name(registry, block)
                        .to_lowercase()
                        .contains(&search)
                    || block.id.to_string().contains(&search)
            })
            .collect();
        let mut selected = None;
        let mut cancel = false;
        let mut open = self.open;
        egui::Window::new("Link existing block")
            .collapsible(false)
            .resizable(true)
            .default_width(420.0)
            .open(&mut open)
            .show(context, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text("Search by name or UUID"),
                )
                .request_focus();
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if blocks.is_empty() {
                            ui.weak(if search.is_empty() {
                                "No blocks are available to link."
                            } else {
                                "No matching blocks."
                            });
                        }
                        for block in &blocks {
                            let label = cached_display_name(registry, block);
                            if ui
                                .button(label)
                                .on_hover_text(block.id.to_string())
                                .clicked()
                            {
                                selected = Some(block.clone());
                            }
                        }
                    });
                ui.separator();
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        if selected.is_some() || cancel {
            open = false;
        }
        self.open = open;
        selected
    }

    pub fn handle(
        &mut self,
        context: &egui::Context,
        editors: &mut EditorAccess<'_>,
        created_parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut result = match self.pending_action.take() {
            Some(BlockPickerMenuAction::New(block_type)) => {
                self.create_registered_block(editors, block_type, created_parent)
            }
            Some(BlockPickerMenuAction::LinkExisting) => {
                self.open = true;
                self.search.clear();
                None
            }
            None => None,
        };
        if result.is_none() {
            result = self.show_creation_options(context, editors, created_parent);
        }
        if result.is_none() {
            if let Some(block) =
                self.show_link_picker(context, editors.client(), editors.registry())
            {
                editors.ensure(block.id, block.block_type);
                result = Some(BlockPickerResult {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
                    properties: block.properties,
                });
            }
        }
        self.show_error(context);
        result
    }

    fn create_registered_block(
        &mut self,
        editors: &mut EditorAccess<'_>,
        block_type: Uuid,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        match editors.registry().create(editors.client(), block_type) {
            Some(BlockCreation::Created(editor)) => {
                Some(Self::finish_creation(editors, editor, block_type, parent))
            }
            // The block needs options first, so the dialog takes over.
            Some(BlockCreation::Options(creation)) => {
                self.pending_block = Some(PendingBlock {
                    block_type,
                    creation,
                });
                None
            }
            None => {
                self.error = Some(format!("Could not create block type {block_type}"));
                None
            }
        }
    }

    /// The dialog for a block type that cannot be created until the user
    /// fills something in.
    fn show_creation_options(
        &mut self,
        context: &egui::Context,
        editors: &mut EditorAccess<'_>,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let mut pending = self.pending_block.take()?;
        let title = editors
            .registry()
            .display_name(pending.block_type)
            .unwrap_or("block");
        let mut create = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new(format!("New {title}"))
            .id(egui::Id::new(("block-picker-create", self.id)))
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                let ready = pending.creation.ui(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    create = ui
                        .add_enabled(ready, egui::Button::new("Create"))
                        .on_disabled_hover_text("Fill in the options first")
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        if cancel || !open {
            return None;
        }
        if !create {
            self.pending_block = Some(pending);
            return None;
        }
        match pending.creation.create(editors.client()) {
            Ok(editor) => Some(Self::finish_creation(
                editors,
                editor,
                pending.block_type,
                parent,
            )),
            Err(error) => {
                self.error = Some(error);
                None
            }
        }
    }

    fn finish_creation(
        editors: &mut EditorAccess<'_>,
        editor: Box<dyn BlockEditor>,
        block_type: Uuid,
        parent: BlockParent,
    ) -> BlockPickerResult {
        editor.set_parent(parent);
        let id = editor.id();
        let mut result_properties = BTreeMap::new();
        if let Some(value) = editor.name() {
            result_properties.insert(
                properties::NAME,
                properties::encode_name(&BlockName {
                    manual: false,
                    value,
                }),
            );
        }
        editors.insert(editor);
        BlockPickerResult {
            id,
            block_type,
            author: editors.client().account_id(),
            properties: result_properties,
        }
    }

    fn show_error(&mut self, context: &egui::Context) {
        let Some(error) = self.error.clone() else {
            return;
        };
        egui::Window::new("Block picker error")
            .id(egui::Id::new(("block-picker-error", self.id)))
            .collapsible(false)
            .resizable(false)
            .show(context, |ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.button("Dismiss").clicked() {
                    self.error = None;
                }
            });
    }
}
