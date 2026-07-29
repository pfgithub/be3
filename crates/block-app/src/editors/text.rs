use block::{Block, BlockParent};
use block_client::{blocks::text::TextDocument, BlockHandle, BlockRelationships};
use eframe::egui;
use uuid::Uuid;

use super::{BlockEditor, EditorAction};

pub(super) struct TextEditor {
    block: BlockHandle<TextDocument>,
}

impl TextEditor {
    pub(super) fn new(block: BlockHandle<TextDocument>) -> Self {
        Self { block }
    }
}

impl BlockEditor for TextEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        TextDocument::TYPE_ID
    }

    fn name(&self) -> String {
        self.block.name()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.block.read().map(|_| self.block.relationships())
    }

    fn set_parent(&self, parent: BlockParent) {
        self.block.set_parent(parent);
    }

    fn note_backref(&self, id: Uuid) {
        self.block.note_backref(id);
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
    ) -> Option<EditorAction> {
        let Some(document) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let original = document.text();
        drop(document);

        let mut edited = original.clone();
        let response = ui.add_sized(
            ui.available_size(),
            egui::TextEdit::multiline(&mut edited).desired_width(f32::INFINITY),
        );
        if response.changed() {
            apply_text_edit(&self.block, &original, &edited);
        }
        None
    }
}

fn apply_text_edit(block: &BlockHandle<TextDocument>, original: &str, edited: &str) {
    let original: Vec<_> = original.chars().collect();
    let edited: Vec<_> = edited.chars().collect();

    let prefix = original
        .iter()
        .zip(&edited)
        .take_while(|(left, right)| left == right)
        .count();
    let max_suffix = original.len().min(edited.len()) - prefix;
    let suffix = original
        .iter()
        .rev()
        .zip(edited.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();

    for _ in prefix..(original.len() - suffix) {
        let operation = {
            let Some(document) = block.read() else {
                return;
            };
            document.remove_operation(prefix).ok()
        };
        if let Some(operation) = operation {
            block.operate(operation);
        }
    }

    for (offset, character) in edited[prefix..edited.len() - suffix]
        .iter()
        .copied()
        .enumerate()
    {
        let operation = {
            let Some(document) = block.read() else {
                return;
            };
            document.insert_operation(prefix + offset, character).ok()
        };
        if let Some(operation) = operation {
            block.operate(operation);
        }
    }
}
