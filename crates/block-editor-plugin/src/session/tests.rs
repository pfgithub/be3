use super::*;
use block_plugin_api::{
    EditorRegion, HelloAccepted, InputBatch, ScreenRequest, ScreenSet, ViewportMetrics,
};

fn accept(session: &mut ClientSession) {
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        capabilities: vec![Capability::Lifecycle, Capability::Input],
        dark_theme: true,
    }));
}

fn open(session: &mut ClientSession, instance: EditorInstanceId) {
    session.receive(Message::Editor(block_plugin_api::EditorMessage::Open {
        instance,
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        editable: true,
    }));
}

fn screen(screen: ScreenId, instance: EditorInstanceId) -> ScreenRequest {
    ScreenRequest {
        screen,
        instance,
        region: EditorRegion::Main,
        metrics: ViewportMetrics {
            logical_width: 100.0,
            logical_height: 100.0,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        },
    }
}

mod accepts_ordered_lifecycle;
mod opens_and_closes_editor_instance;
mod rejects_child_statuses_for_unopened_instances;
mod rejects_out_of_order_messages;
mod rejects_screens_for_unopened_instances;
