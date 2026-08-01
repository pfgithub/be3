use block::{Block, BlockParent};
use block_client::{
    blocks::workspace_index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation},
    BlockHandle, BlockRelationships,
};
use eframe::egui;
use egui_material_icons::icons::ICON_FOLDER;
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorRegistration,
};

const DIRECT_EDITOR_WIDTH: f32 = 400.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 24.0;

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: WorkspaceIndex::TYPE_ID,
        display_name: "Folder",
        icon: ICON_FOLDER,
        create: Some(|client| {
            Box::new(WorkspaceIndexEditor::new(
                client.create_block(WorkspaceIndex::default()),
            ))
        }),
        open: |client, id| {
            Box::new(WorkspaceIndexEditor::new(
                client.get_block::<WorkspaceIndex>(id),
            ))
        },
        can_add_child: true,
        can_delete_child: true,
        regenerate_dynamic_artifact: None,
    }
}

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

    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?;
        let present = index
            .entries()
            .iter()
            .any(|existing| existing.id == entry.id);
        drop(index);
        if present {
            self.block.operate(WorkspaceIndexOperation::Remove(entry));
        }
        Some(true)
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
        let entry_count = self.block.read()?.entries().len().max(1);
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_ROW_HEIGHT * entry_count as f32,
        ))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
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

        for entry in index.entries() {
            ui.label(entry.id.to_string());
        }
        None
    }
}
