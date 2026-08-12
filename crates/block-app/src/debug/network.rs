use block::{ClientMessage, ServerMessage};
use block_client::{NetworkDirection, NetworkTrafficEntry};
use eframe::egui;
use egui_material_icons::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_PAUSE, ICON_PLAY_ARROW, ICON_SKIP_NEXT,
};
use serde_json::Value;

use crate::BlockApp;

impl BlockApp {
    pub(crate) fn show_network_debug(&mut self, ctx: &egui::Context) {
        if !self.network_debug_open {
            return;
        }
        self.client.enable_network_traffic_logging();
        let debug = self.client.network_debug_snapshot();
        let mut open = self.network_debug_open;
        egui::Window::new("Network Traffic")
            .open(&mut open)
            .default_size([720.0, 480.0])
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!debug.sending_paused, egui::Button::new(ICON_PAUSE))
                        .on_hover_text("Pause sending")
                        .clicked()
                    {
                        self.client.pause_sending();
                    }
                    if ui
                        .add_enabled(
                            debug.sending_paused && debug.queued_messages > 0,
                            egui::Button::new(ICON_SKIP_NEXT),
                        )
                        .on_hover_text("Send the next queued message")
                        .clicked()
                    {
                        self.client.step_sending();
                    }
                    if ui
                        .add_enabled(debug.sending_paused, egui::Button::new(ICON_PLAY_ARROW))
                        .on_hover_text("Resume sending")
                        .clicked()
                    {
                        self.client.resume_sending();
                    }
                    if ui.button("Clear").clicked() {
                        self.client.clear_network_traffic();
                    }
                    ui.separator();
                    ui.small(if debug.sending_paused {
                        format!("Paused \u{2022} {} queued", debug.queued_messages)
                    } else {
                        format!("Sending \u{2022} {} queued", debug.queued_messages)
                    });
                });
                ui.separator();
                let row_height = ui.text_style_height(&egui::TextStyle::Monospace) * 8.0;
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show_rows(ui, row_height, debug.traffic.len(), |ui, rows| {
                        if debug.traffic.is_empty() {
                            ui.weak("No network traffic yet");
                        }
                        for entry in &debug.traffic[rows] {
                            show_traffic_entry(ui, entry);
                        }
                    });
            });
        self.network_debug_open = open;
    }
}

fn show_traffic_entry(ui: &mut egui::Ui, entry: &NetworkTrafficEntry) {
    let (arrow, color) = match entry.direction {
        NetworkDirection::Sent => (ICON_ARROW_FORWARD, ui.visuals().hyperlink_color),
        NetworkDirection::Received => (ICON_ARROW_BACK, ui.visuals().warn_fg_color),
    };
    ui.horizontal_top(|ui| {
        ui.label(arrow.rich_text().color(color));
        ui.small(format!("{}", entry.timestamp_ms));
        ui.add(
            egui::Label::new(egui::RichText::new(&entry.payload).monospace())
                .wrap()
                .selectable(true),
        );
    });
    for decoded in decoded_payloads(entry) {
        ui.horizontal_top(|ui| {
            ui.add_space(56.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(decoded)
                        .monospace()
                        .color(ui.visuals().weak_text_color()),
                )
                .wrap()
                .selectable(true),
            );
        });
    }
}

fn decoded_payloads(entry: &NetworkTrafficEntry) -> Vec<String> {
    match entry.direction {
        NetworkDirection::Sent => serde_json::from_str::<ClientMessage>(&entry.payload)
            .ok()
            .map_or_else(Vec::new, decoded_client_message),
        NetworkDirection::Received => serde_json::from_str::<ServerMessage>(&entry.payload)
            .ok()
            .map_or_else(Vec::new, decoded_server_message),
    }
}

fn decoded_client_message(message: ClientMessage) -> Vec<String> {
    match message {
        ClientMessage::UpdateBlock { id, operation, .. } => {
            decoded_operation(id.to_string(), &operation)
                .into_iter()
                .collect()
        }
        ClientMessage::UpdateBatch { updates, .. } => updates
            .into_iter()
            .filter_map(|update| decoded_operation(update.id.to_string(), &update.operation))
            .collect(),
        ClientMessage::SetPresence {
            id,
            presence_id,
            data: Some(data),
            ..
        } => decoded_presence(id.to_string(), presence_id.to_string(), &data)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn decoded_server_message(message: ServerMessage) -> Vec<String> {
    match message {
        ServerMessage::ReadBlock { id, operations, .. } => operations
            .into_iter()
            .filter_map(|operation| decoded_operation(id.to_string(), &operation.operation))
            .collect(),
        ServerMessage::BatchOk { operations, .. } | ServerMessage::BatchUpdated { operations } => {
            operations
                .into_iter()
                .filter_map(|operation| {
                    decoded_operation(operation.id.to_string(), &operation.operation.operation)
                })
                .collect()
        }
        ServerMessage::BlockUpdated { id, operation, .. } => {
            decoded_operation(id.to_string(), &operation.operation)
                .into_iter()
                .collect()
        }
        ServerMessage::Presence {
            id,
            presence_id,
            data: Some(data),
            ..
        } => decoded_presence(id.to_string(), presence_id.to_string(), &data)
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

fn decoded_operation(id: String, data: &[u8]) -> Option<String> {
    decoded_data(data).map(|data| format!("Operation for block {id}:\n{data}"))
}

fn decoded_presence(id: String, presence_id: String, data: &[u8]) -> Option<String> {
    decoded_data(data).map(|data| format!("Presence {presence_id} for block {id}:\n{data}"))
}

fn decoded_data(data: &[u8]) -> Option<String> {
    serde_json::from_slice::<Value>(data)
        .ok()
        .and_then(|data| serde_json::to_string_pretty(&data).ok())
}
