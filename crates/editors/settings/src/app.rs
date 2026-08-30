use std::sync::Arc;

use block::{Block, BlockParent};
use block_client::block_ref::BlockRef;
use block_client::blocks::settings::{ActivationCondition, Settings, SettingsOperation};
use block_client::blocks::ui_settings::UiSettings;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, EditorHost};

const INTRINSIC_WIDTH: f32 = 360.0;
const INTRINSIC_HEIGHT: f32 = 120.0;

#[derive(Default)]
pub struct SettingsApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<Settings>>,
}

impl SettingsApp {
    fn ui_settings(&self) -> Option<uuid::Uuid> {
        let host = self.host.as_ref()?;
        let settings = self.block.as_ref()?.read()?;
        settings
            .resolve(UiSettings::TYPE_ID, host.client_id())
            .and_then(|reference| reference.as_direct())
    }

    fn create_ui_settings(&self) -> Option<uuid::Uuid> {
        let client = self.client.as_ref()?;
        let settings = self.block.as_ref()?;
        let block = client.create_block(UiSettings::new());
        settings.operate(SettingsOperation::SetEntry {
            block_type: UiSettings::TYPE_ID,
            activation: ActivationCondition::Fallback,
            block: BlockRef::Direct(block.id()),
        });
        block.set_parent(BlockParent::Uuid(settings.id()));
        Some(block.id())
    }
}

impl block_editor_plugin::App for SettingsApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: uuid::Uuid) {
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(egui::vec2(INTRINSIC_WIDTH, INTRINSIC_HEIGHT))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(host) = self.host.clone() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        if self.block.as_ref().and_then(BlockHandle::read).is_none() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        if !ui
            .add_enabled(host.editable(), egui::Button::new("UI settings"))
            .test_id("settings.ui-settings")
            .clicked()
        {
            return;
        }
        if let Some(id) = self.ui_settings().or_else(|| self.create_ui_settings()) {
            host.open_block(id, UiSettings::TYPE_ID);
        }
    }
}
