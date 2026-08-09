use block::BlockParent;
use block_client::{
    properties::BlockName, BlockHandleAccess, BlockHistoryHandle, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
};

const DIRECT_EDITOR_SIZE: egui::Vec2 = egui::vec2(400.0, 120.0);

pub(super) struct UnsupportedEditor {
    id: Uuid,
    block_type: Uuid,
}

impl UnsupportedEditor {
    pub(super) fn new(id: Uuid, block_type: Uuid) -> Self {
        Self { id, block_type }
    }
}

impl BlockHandleAccess for UnsupportedEditor {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type(&self) -> Uuid {
        self.block_type
    }

    fn name(&self) -> Option<String> {
        Some(self.id.to_string())
    }

    fn block_name(&self) -> Option<BlockName> {
        Some(BlockName {
            manual: true,
            value: self.id.to_string(),
        })
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        None
    }

    fn set_parent(&self, _parent: BlockParent) {}

    fn history(&self) -> Option<&dyn BlockHistoryHandle> {
        None
    }
}

impl BlockEditor for UnsupportedEditor {
    fn block(&self) -> &dyn BlockHandleAccess {
        self
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        Some(DIRECT_EDITOR_SIZE)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
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
