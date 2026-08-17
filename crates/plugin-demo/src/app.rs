use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static RUNNER: RefCell<Option<eframe::WebRunner>> = const { RefCell::new(None) };
}

impl eframe::App for crate::demo::Demo {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        self.show(ui);
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

    let runner = eframe::WebRunner::new();
    runner
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|_| Ok(Box::<crate::demo::Demo>::default())),
        )
        .await?;
    RUNNER.with(|current| current.replace(Some(runner)));
    Ok(())
}

#[wasm_bindgen]
pub fn shutdown() {
    RUNNER.with(|current| {
        if let Some(runner) = current.borrow_mut().take() {
            runner.destroy();
        }
    });
}
