use std::sync::Arc;

use block_client::blocks::ui_settings::{UiSettings, UiSettingsOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, EditorHost};

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 120.0;
const MIN_ZOOM: f32 = 0.5;
const MAX_ZOOM: f32 = 3.0;

#[derive(Default)]
pub struct UiSettingsApp {
    host: Option<EditorHost>,
    block: Option<BlockHandle<UiSettings>>,
}

impl UiSettingsApp {
    fn editable(&self) -> bool {
        self.host.as_ref().is_none_or(EditorHost::editable)
    }
}

impl block_editor_plugin::App for UiSettingsApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: uuid::Uuid) {
        self.host = Some(host);
        self.block = Some(client.get_block(block_id));
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(egui::vec2(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(block) = self.block.clone() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(settings) = block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let mut zoom = settings.zoom();
        drop(settings);

        ui.horizontal(|ui| {
            ui.label("Zoom");
            let slider = ui.add_enabled(
                self.editable(),
                egui::Slider::new(&mut zoom, MIN_ZOOM..=MAX_ZOOM).suffix("x"),
            );
            if slider.test_id("ui-settings.zoom").changed() {
                block.operate(UiSettingsOperation::SetZoom { zoom });
            }
        });
    }
}
