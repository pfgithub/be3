use std::collections::HashSet;

use block_client::{BlockClient, CachedBlock};
use eframe::egui;
use uuid::Uuid;

use crate::editors::EditorRegistry;

pub enum BlockPickerMenuAction {
    New(Uuid),
    LinkExisting,
}

#[derive(Default)]
pub struct BlockPicker {
    open: bool,
    search: String,
    excluded: HashSet<Uuid>,
}

impl BlockPicker {
    pub fn show_menu(
        ui: &mut egui::Ui,
        registry: &EditorRegistry,
    ) -> Option<BlockPickerMenuAction> {
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
        action
    }

    pub fn open(&mut self, excluded: impl IntoIterator<Item = Uuid>) {
        self.open = true;
        self.search.clear();
        self.excluded = excluded.into_iter().collect();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn show(&mut self, context: &egui::Context, client: &BlockClient) -> Option<CachedBlock> {
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
}
