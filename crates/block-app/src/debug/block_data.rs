use eframe::egui;
use uuid::Uuid;

use crate::BlockApp;

impl BlockApp {
    /// Shows a tab's block as its raw serialized data instead of its editor.
    pub(crate) fn show_debug_data(&mut self, ui: &mut egui::Ui, active: Uuid) {
        let data = self.client.block_debug_data(active);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| match &data {
                Some(data) => {
                    ui.add(
                        egui::Label::new(egui::RichText::new(data).monospace())
                            .wrap()
                            .selectable(true),
                    );
                }
                None => {
                    ui.weak("This block has not finished loading yet.");
                }
            });
    }
}
