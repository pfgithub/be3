use block_client::{
    blocks::counter::{Counter, CounterOperation},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_123, ICON_ADD, ICON_REMOVE},
    MaterialIcon,
};

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

impl EditorKind for CounterEditor {
    type Block = Counter;

    const DISPLAY_NAME: &'static str = "Counter";
    const ICON: MaterialIcon = ICON_123;

    fn open(_client: &BlockClient, block: BlockHandle<Counter>) -> Self {
        Self { block }
    }
}

impl CreatableEditor for CounterEditor {
    fn create(client: &BlockClient) -> Self {
        Self {
            block: client.create_block(Counter::new()),
        }
    }
}

pub(super) struct CounterEditor {
    block: BlockHandle<Counter>,
}

impl BlockEditor for CounterEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
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
        Some(egui::vec2(240.0, 80.0))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(counter) = self.block.read() else {
            ui.spinner();
            return None;
        };
        let count = counter.count();
        drop(counter);

        ui.centered_and_justified(|ui| {
            ui.horizontal(|ui| {
                if ui.button(ICON_REMOVE).clicked() {
                    self.block.operate(CounterOperation::Decrement);
                }
                ui.label(count.to_string());
                if ui.button(ICON_ADD).clicked() {
                    self.block.operate(CounterOperation::Increment);
                }
            });
        });
        None
    }
}
