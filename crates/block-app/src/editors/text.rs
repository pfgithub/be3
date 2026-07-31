use block::{Block, BlockParent};
use block_client::{
    blocks::text::{TextDocument, TextOperation},
    BlockHandle, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use super::{BlockEditor, EditorAccess, EditorAction};

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

    fn history(&self) -> Option<&dyn block_client::BlockHistoryHandle> {
        Some(&self.block)
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
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
            if let Some(document) = self.block.read() {
                let operations = text_edit_operations(&document, &original, &edited);
                drop(document);
                self.block.operate_grouped(operations);
            }
        } else if response.lost_focus() {
            self.block.finish_history_group();
        }
        None
    }
}

fn text_edit_operations(
    document: &TextDocument,
    original: &str,
    edited: &str,
) -> Vec<TextOperation> {
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
    let original_end = original.len() - suffix;
    let edited_end = edited.len() - suffix;
    if prefix == original_end && prefix == edited_end {
        return Vec::new();
    }

    let mut document = document.clone();
    let mut operations = Vec::new();
    for _ in prefix..original_end {
        let Ok(operation) = document.remove_operation(prefix) else {
            return Vec::new();
        };
        TextDocument::apply_operation(&mut document, &operation);
        operations.push(operation);
    }
    for (offset, character) in edited[prefix..edited_end].iter().copied().enumerate() {
        let Ok(operation) = document.insert_operation(prefix + offset, character) else {
            return Vec::new();
        };
        TextDocument::apply_operation(&mut document, &operation);
        operations.push(operation);
    }
    operations
}
