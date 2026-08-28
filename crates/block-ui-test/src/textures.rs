use std::sync::{Arc, Mutex};

use egui_kittest::TestRenderer;
use paint_snapshot::TextureStore;

#[derive(Clone, Default)]
pub struct Textures(Arc<Mutex<TextureStore>>);

impl Textures {
    pub fn store(&self) -> std::sync::MutexGuard<'_, TextureStore> {
        self.0.lock().expect("the texture store was poisoned")
    }
}

impl TestRenderer for Textures {
    fn handle_delta(&mut self, delta: &egui::TexturesDelta) {
        self.store().apply(delta);
    }
}
