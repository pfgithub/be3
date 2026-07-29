use std::collections::HashSet;

use block_client::{BlockClient, CachedBlock};
use eframe::egui;
use uuid::Uuid;

#[derive(Default)]
pub struct BlockPicker {
    open: bool,
    input: String,
    excluded: HashSet<Uuid>,
}

impl BlockPicker {
    pub fn open(&mut self, excluded: impl IntoIterator<Item = Uuid>) {
        self.open = true;
        self.input.clear();
        self.excluded = excluded.into_iter().collect();
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    pub fn show(&mut self, context: &egui::Context, client: &BlockClient) -> Option<CachedBlock> {
        if !self.open {
            return None;
        }

        let parsed = self.input.trim().parse::<Uuid>().ok();
        let cached = parsed
            .filter(|id| !self.excluded.contains(id))
            .and_then(|id| client.cached_block(id));
        let mut selected = None;
        let mut cancel = false;
        let mut open = self.open;
        egui::Window::new("Choose cached block")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .show(context, |ui| {
                ui.label("Block UUID");
                ui.text_edit_singleline(&mut self.input).request_focus();

                match (parsed, cached.as_ref()) {
                    (None, _) if !self.input.trim().is_empty() => {
                        ui.colored_label(ui.visuals().error_fg_color, "Enter a valid UUID.");
                    }
                    (Some(id), _) if self.excluded.contains(&id) => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            "This block cannot be referenced here.",
                        );
                    }
                    (Some(_), None) => {
                        ui.colored_label(
                            ui.visuals().error_fg_color,
                            "That UUID is not in the client cache.",
                        );
                    }
                    (_, Some(block)) => {
                        ui.label(if block.name.is_empty() {
                            block.id.to_string()
                        } else {
                            format!("{} ({})", block.name, block.id)
                        });
                    }
                    _ => {
                        ui.weak(format!(
                            "{} cached blocks available",
                            client.cached_blocks().len()
                        ));
                    }
                }

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(cached.is_some(), egui::Button::new("Place block"))
                        .clicked()
                    {
                        selected = cached.clone();
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
            });
        if selected.is_some() || cancel {
            open = false;
        }
        self.open = open;
        selected
    }
}
