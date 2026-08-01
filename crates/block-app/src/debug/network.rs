use block_client::{NetworkDirection, NetworkTrafficEntry};
use eframe::egui;
use egui_material_icons::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_PAUSE, ICON_PLAY_ARROW, ICON_SKIP_NEXT,
};

use crate::BlockApp;

impl BlockApp {
    pub(crate) fn show_network_debug(&mut self, ctx: &egui::Context) {
        if !self.network_debug_open {
            return;
        }
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
                    ui.separator();
                    ui.small(if debug.sending_paused {
                        format!("Paused \u{2022} {} queued", debug.queued_messages)
                    } else {
                        format!("Sending \u{2022} {} queued", debug.queued_messages)
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        if debug.traffic.is_empty() {
                            ui.weak("No network traffic yet");
                        }
                        for entry in &debug.traffic {
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
}
