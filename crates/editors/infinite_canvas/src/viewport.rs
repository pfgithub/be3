use block_editor_plugin::{egui, EditorHost, ViewChange};

pub(crate) struct Viewport {
    host: EditorHost,
    zoom: f32,
}

impl Viewport {
    pub(crate) fn new(host: EditorHost, zoom: f32) -> Self {
        Self { host, zoom }
    }

    pub(crate) fn zoom(&self) -> f32 {
        self.zoom
    }

    pub(crate) fn pan(&mut self, delta: egui::Vec2) {
        self.host.pan_view(delta);
    }

    pub(crate) fn change_zoom(&mut self, factor: f32, anchor: Option<egui::Pos2>) {
        self.host.zoom_view(factor, anchor);
    }

    pub(crate) fn fit(&mut self) {
        self.host.fit_view();
    }

    pub(crate) fn resume_auto_fit(&mut self) {
        self.host.resume_auto_fit_view();
    }

    pub(crate) fn apply(&mut self, change: ViewChange) {
        match change {
            ViewChange::Pan { x, y } => self.pan(egui::vec2(x, y)),
            ViewChange::Zoom { factor, anchor } => {
                self.change_zoom(factor, anchor.map(|(x, y)| egui::pos2(x, y)))
            }
            ViewChange::Fit => self.fit(),
            ViewChange::ResumeAutoFit => self.resume_auto_fit(),
        }
    }
}
