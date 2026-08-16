use block_client::{
    blocks::ui_settings::{UiSettings, UiSettingsOperation},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_DISPLAY_SETTINGS, MaterialIcon};

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 360.0;
const DIRECT_EDITOR_HEIGHT: f32 = 120.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;

pub(super) struct UiSettingsEditor {
    block: BlockHandle<UiSettings>,
}

impl EditorKind for UiSettingsEditor {
    type Block = UiSettings;

    const DISPLAY_NAME: &'static str = "UI Settings";
    const ICON: MaterialIcon = ICON_DISPLAY_SETTINGS;

    fn open(_client: &BlockClient, block: BlockHandle<UiSettings>) -> Self {
        Self { block }
    }
}

impl BlockEditor for UiSettingsEditor {
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
        Some(egui::vec2(DIRECT_EDITOR_WIDTH, DIRECT_EDITOR_HEIGHT))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(settings) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let mut zoom = settings.zoom();
        drop(settings);

        ui.horizontal(|ui| {
            ui.label("Zoom");
            if ui
                .add(egui::Slider::new(&mut zoom, MIN_ZOOM..=MAX_ZOOM).suffix("x"))
                .changed()
            {
                self.block.operate(UiSettingsOperation::SetZoom { zoom });
            }
        });
        None
    }
}
