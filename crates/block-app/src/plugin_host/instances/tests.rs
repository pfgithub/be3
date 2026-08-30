use super::*;
use crate::plugin_host::EditorBlock;
use block_plugin_api::InputEvent;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Main;
const PASS: u64 = 1;
const SIZE: egui::Vec2 = egui::vec2(100.0, 100.0);

fn placed() -> (Instances, egui::Context, egui::Id) {
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), SIZE);
    let id = egui::Id::new("plugin screen");
    let _ = context.run_ui(egui::RawInput::default(), |ui| {
        ui.interact(rect, id, egui::Sense::click_and_drag());
    });
    let mut instances = Instances::default();
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    let block_types = Arc::new(Vec::new());
    let role = InstanceRole::Editor(EditorBlock {
        id: Uuid::nil(),
        block_type: Uuid::nil(),
    });
    instances.report(
        INSTANCE,
        REGION,
        &context,
        &client,
        Uuid::nil(),
        role,
        &block_types,
        SIZE,
        egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(
        INSTANCE,
        REGION,
        Placement {
            id,
            rect,
            clip: rect,
            pass: PASS,
        },
    );
    (instances, context, id)
}

mod a_plugin_reaches_only_the_hosts_its_manifest_names;
mod input_is_withheld_from_screens_the_plugin_no_longer_has;
