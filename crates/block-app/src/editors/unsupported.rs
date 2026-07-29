use block::BlockParent;
use block_client::BlockRelationships;
use eframe::egui;
use uuid::Uuid;

use super::{BlockEditor, EditorAction};

pub(super) struct UnsupportedEditor {
    id: Uuid,
    block_type: Uuid,
}

impl UnsupportedEditor {
    pub(super) fn new(id: Uuid, block_type: Uuid) -> Self {
        Self { id, block_type }
    }
}

impl BlockEditor for UnsupportedEditor {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type(&self) -> Uuid {
        self.block_type
    }

    fn name(&self) -> String {
        self.id.to_string()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        None
    }

    fn set_parent(&self, _parent: BlockParent) {}

    fn note_backref(&self, _id: Uuid) {}

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Unsupported block type");
                ui.label(format!("Block: {}", self.id));
                ui.label(format!("Type: {}", self.block_type));
            });
        });
        None
    }
}
