use block_client::{NetworkDirection, NetworkTrafficEntry};
use eframe::egui;

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
                        .add_enabled(!debug.sending_paused, egui::Button::new("Pause"))
                        .clicked()
                    {
                        self.client.pause_sending();
                    }
                    if ui
                        .add_enabled(
                            debug.sending_paused && debug.queued_messages > 0,
                            egui::Button::new("Step"),
                        )
                        .on_hover_text("Send the next queued message")
                        .clicked()
                    {
                        self.client.step_sending();
                    }
                    if ui
                        .add_enabled(debug.sending_paused, egui::Button::new("Resume"))
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
        NetworkDirection::Sent => ("\u{2192}", ui.visuals().hyperlink_color),
        NetworkDirection::Received => ("\u{2190}", ui.visuals().warn_fg_color),
    };
    ui.horizontal_top(|ui| {
        ui.colored_label(color, arrow);
        ui.small(format!("{}", entry.timestamp_ms));
        ui.add(
            egui::Label::new(egui::RichText::new(&entry.payload).monospace())
                .wrap()
                .selectable(true),
        );
    });
}
