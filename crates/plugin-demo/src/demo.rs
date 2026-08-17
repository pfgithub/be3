use eframe::egui;

#[derive(Default)]
pub(crate) struct Demo {
    clicks: u32,
    enabled: bool,
    value: f32,
}

impl Demo {
    pub(crate) fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("egui plugin demo");
        ui.label("This interface is rendered by plugin-demo.");
        ui.separator();
        ui.checkbox(&mut self.enabled, "Enable controls");
        ui.add_enabled_ui(self.enabled, |ui| {
            ui.add(egui::Slider::new(&mut self.value, 0.0..=1.0).text("Value"));
            if ui.button("Click me").clicked() {
                self.clicks += 1;
            }
        });
        ui.label(format!("Clicks: {}", self.clicks));
    }
}
