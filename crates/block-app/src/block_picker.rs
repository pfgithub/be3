use std::collections::HashSet;

use block::{Block, BlockParent};
use block_client::blocks::image::Image;
use block_client::{BlockClient, CachedBlock};
use eframe::egui;
use uuid::Uuid;

use crate::editors::{
    image::{pick_image_file, ImageEditor},
    EditorAccess, EditorRegistry,
};

enum BlockPickerMenuAction {
    New(Uuid),
    ImportImage,
    LinkExisting,
}

pub struct BlockPickerResult {
    pub id: Uuid,
    pub block_type: Uuid,
    pub author: Uuid,
    pub name: String,
    imported_image: Option<ImportedImage>,
}

pub struct ImportedImage {
    pub source_name: String,
    pub width: u32,
    pub height: u32,
}

impl BlockPickerResult {
    pub fn imported_image(&self) -> Option<&ImportedImage> {
        self.imported_image.as_ref()
    }
}

pub struct BlockPicker {
    id: Uuid,
    open: bool,
    search: String,
    excluded: HashSet<Uuid>,
    pending_action: Option<BlockPickerMenuAction>,
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
            if ui.button("Image").clicked() {
                action = Some(BlockPickerMenuAction::ImportImage);
                ui.close();
            }
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

    pub fn open(&mut self, excluded: impl IntoIterator<Item = Uuid>) {
        self.open = true;
        self.search.clear();
        self.excluded = excluded.into_iter().collect();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    fn show_link_picker(
        &mut self,
        context: &egui::Context,
        client: &BlockClient,
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
                    || block.name.to_lowercase().contains(&search)
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
                            let label = if block.name.is_empty() {
                                block.id.to_string()
                            } else {
                                block.name.clone()
                            };
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
            Some(BlockPickerMenuAction::ImportImage) => self.import_image(editors, created_parent),
            Some(BlockPickerMenuAction::LinkExisting) => {
                self.open = true;
                self.search.clear();
                None
            }
            None => None,
        };
        if result.is_none() {
            if let Some(block) = self.show_link_picker(context, editors.client()) {
                editors.ensure(block.id, block.block_type);
                result = Some(BlockPickerResult {
                    id: block.id,
                    block_type: block.block_type,
                    author: block.author,
                    name: block.name,
                    imported_image: None,
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
        let Some(editor) = editors.registry().create(editors.client(), block_type) else {
            self.error = Some(format!("Could not create block type {block_type}"));
            return None;
        };
        editor.set_parent(parent);
        let id = editor.id();
        let name = editor.name();
        editors.insert(editor);
        let author = editors.client().account_id();
        Some(BlockPickerResult {
            id,
            block_type,
            author,
            name,
            imported_image: None,
        })
    }

    fn import_image(
        &mut self,
        editors: &mut EditorAccess<'_>,
        parent: BlockParent,
    ) -> Option<BlockPickerResult> {
        let image = match pick_image_file() {
            Ok(Some(image)) => image,
            Ok(None) => return None,
            Err(error) => {
                self.error = Some(error);
                return None;
            }
        };
        let imported_image = ImportedImage {
            source_name: image.source_name().to_owned(),
            width: image.width(),
            height: image.height(),
        };
        let block = editors.client().create_block(image);
        block.set_parent(parent);
        let id = block.id();
        let name = block.name();
        let author = editors.client().account_id();
        editors.insert(Box::new(ImageEditor::new(block)));
        Some(BlockPickerResult {
            id,
            block_type: Image::TYPE_ID,
            author,
            name,
            imported_image: Some(imported_image),
        })
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
