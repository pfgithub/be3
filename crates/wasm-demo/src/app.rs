use eframe::egui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Default)]
struct Demo {
    clicks: u32,
    enabled: bool,
    value: f32,
}

impl eframe::App for Demo {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("egui wasm demo");
        ui.label("This interface is rendered by wasm-demo.");
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

#[wasm_bindgen]
pub async fn start(canvas_id: String) -> Result<(), JsValue> {
    let window =
        web_sys::window().ok_or_else(|| JsValue::from_str("no browser window is available"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document is available"))?;
    let canvas = document
        .get_element_by_id(&canvas_id)
        .ok_or_else(|| JsValue::from_str(&format!("no element id {canvas_id}")))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::<Demo>::default())),
        )
        .await
}
