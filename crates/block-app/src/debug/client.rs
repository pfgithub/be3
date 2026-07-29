use block_client::{
    BlockDebugSnapshot, ClientDebugEntry, ClientDebugSnapshot, PendingRequestDebugSnapshot,
};
use eframe::egui;

use crate::BlockApp;

impl BlockApp {
    pub(crate) fn show_client_debug(&mut self, ctx: &egui::Context) {
        if !self.client_debug_open {
            return;
        }
        let snapshot = self.client.client_debug_snapshot();
        let mut open = self.client_debug_open;
        egui::Window::new("Block Client State")
            .open(&mut open)
            .default_size([760.0, 600.0])
            .show(ctx, |ui| {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| show_snapshot(ui, &snapshot));
            });
        self.client_debug_open = open;
    }
}

fn show_snapshot(ui: &mut egui::Ui, snapshot: &ClientDebugSnapshot) {
    egui::Grid::new("client-debug-summary")
        .num_columns(2)
        .striped(true)
        .show(ui, |ui| {
            field(ui, "Client", snapshot.client_id);
            field(
                ui,
                "Connection",
                if snapshot.connected {
                    "Connected"
                } else {
                    "Disconnected"
                },
            );
            field(
                ui,
                "Changes",
                if snapshot.changes_saved {
                    "Saved"
                } else {
                    "Pending"
                },
            );
            field(
                ui,
                "Sending",
                if snapshot.sending_paused {
                    "Paused"
                } else {
                    "Active"
                },
            );
            field(ui, "Queued messages", snapshot.queued_messages);
            field(ui, "Steps remaining", snapshot.steps_remaining);
            field(
                ui,
                "Synchronization waiters",
                snapshot.synchronization_waiters,
            );
        });
    ui.separator();

    egui::CollapsingHeader::new(format!("Blocks ({})", snapshot.blocks.len()))
        .default_open(true)
        .show(ui, |ui| {
            if snapshot.blocks.is_empty() {
                empty(ui);
            }
            for block in &snapshot.blocks {
                show_block(ui, block);
            }
        });

    egui::CollapsingHeader::new(format!(
        "Reference Watches ({})",
        snapshot.reference_lists.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        if snapshot.reference_lists.is_empty() {
            empty(ui);
        }
        for reference in &snapshot.reference_lists {
            egui::Grid::new(("client-debug-reference", reference.list))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    field(ui, "List", format!("{:?}", reference.list));
                    field(ui, "Loaded", yes_no(reference.loaded));
                    field(ui, "Blocks", reference.blocks);
                });
            ui.add_space(4.0);
        }
    });

    egui::CollapsingHeader::new(format!("Cache ({})", snapshot.cached_blocks.len())).show(
        ui,
        |ui| {
            if snapshot.cached_blocks.is_empty() {
                empty(ui);
            }
            for block in &snapshot.cached_blocks {
                ui.horizontal(|ui| {
                    monospace(ui, &block.name);
                    ui.weak("·");
                    monospace(ui, block.id);
                    ui.weak("·");
                    monospace(ui, block.block_type);
                });
            }
        },
    );

    egui::CollapsingHeader::new(format!(
        "Pending Requests ({})",
        snapshot.pending_requests.len()
    ))
    .default_open(true)
    .show(ui, |ui| {
        if snapshot.pending_requests.is_empty() {
            empty(ui);
        }
        for request in &snapshot.pending_requests {
            show_pending_request(ui, request);
        }
    });

    show_entries(
        ui,
        "Outbound Queue",
        "client-debug-outbound",
        &snapshot.outbound_messages,
    );
    show_entries(
        ui,
        "Deferred Queue",
        "client-debug-deferred",
        &snapshot.deferred_requests,
    );
}

fn show_block(ui: &mut egui::Ui, block: &BlockDebugSnapshot) {
    let name = if block.name.is_empty() {
        "(unnamed)"
    } else {
        &block.name
    };
    egui::CollapsingHeader::new(format!("{name} · {}", block.id))
        .id_salt(("client-debug-block", block.id))
        .show(ui, |ui| {
            egui::Grid::new(("client-debug-block-fields", block.id))
                .num_columns(2)
                .striped(true)
                .show(ui, |ui| {
                    field(ui, "ID", block.id);
                    field(ui, "Type", block.block_type);
                    field(ui, "Name", &block.name);
                    field(ui, "CRDT", yes_no(block.crdt));
                    field(ui, "Ready", yes_no(block.ready));
                    field(ui, "Synchronized", yes_no(block.synchronized));
                    field(ui, "Local changes", yes_no(block.has_local_changes));
                    field(ui, "Confirmed sequence", block.confirmed_seq);
                    field(ui, "Acknowledged sequence", block.acknowledged_seq);
                    field(ui, "Pending operations", block.pending_operations);
                    field(ui, "In-flight operations", block.in_flight_operations);
                    field(ui, "Buffered operations", block.buffered_operations);
                });
        });
}

fn show_pending_request(ui: &mut egui::Ui, request: &PendingRequestDebugSnapshot) {
    ui.horizontal_top(|ui| {
        monospace(ui, request.request_id);
        ui.strong(&request.kind);
        if !request.details.is_empty() {
            monospace(ui, &request.details);
        }
    });
}

fn show_entries(ui: &mut egui::Ui, title: &str, id: &'static str, entries: &[ClientDebugEntry]) {
    egui::CollapsingHeader::new(format!("{title} ({})", entries.len()))
        .default_open(true)
        .show(ui, |ui| {
            if entries.is_empty() {
                empty(ui);
            }
            egui::Grid::new(id)
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    for (index, entry) in entries.iter().enumerate() {
                        monospace(ui, index);
                        ui.strong(&entry.kind);
                        monospace(ui, &entry.details);
                        ui.end_row();
                    }
                });
        });
}

fn field(ui: &mut egui::Ui, label: &str, value: impl ToString) {
    ui.label(label);
    monospace(ui, value);
    ui.end_row();
}

fn monospace(ui: &mut egui::Ui, value: impl ToString) {
    ui.add(
        egui::Label::new(egui::RichText::new(value.to_string()).monospace())
            .wrap_mode(egui::TextWrapMode::Extend)
            .selectable(true),
    );
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "Yes"
    } else {
        "No"
    }
}

fn empty(ui: &mut egui::Ui) {
    ui.weak("None");
}
