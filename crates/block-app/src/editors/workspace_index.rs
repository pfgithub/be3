use block::{Block, BlockParent};
use block_client::{
    blocks::workspace_index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation},
    BlockHandle, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use super::{BlockEditor, EditorAction};

pub(super) struct WorkspaceIndexEditor {
    block: BlockHandle<WorkspaceIndex>,
}

impl WorkspaceIndexEditor {
    pub(super) fn new(block: BlockHandle<WorkspaceIndex>) -> Self {
        Self { block }
    }
}

impl BlockEditor for WorkspaceIndexEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        WorkspaceIndex::TYPE_ID
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

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?;
        let already_present = index
            .entries()
            .iter()
            .any(|existing| existing.id == entry.id);
        drop(index);
        if !already_present {
            self.block.operate(WorkspaceIndexOperation::Add(entry));
        }
        Some(true)
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
    ) -> Option<EditorAction> {
        let Some(index) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };

        if index.entries().is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This folder is empty.");
            });
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in index.entries() {
                ui.label(entry.id.to_string());
            }
        });
        None
    }
}
