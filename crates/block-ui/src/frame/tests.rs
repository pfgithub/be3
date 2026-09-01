use super::*;

#[derive(Default)]
struct Bands {
    toolbar: bool,
    left: bool,
    right: bool,
    content: bool,
}

impl FrameBands for Bands {
    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.toolbar = true;
        ui.label("toolbar");
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.left = true;
        ui.label("left");
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.right = true;
        ui.label("right");
    }

    fn content_ui(&mut self, ui: &mut egui::Ui) {
        self.content = true;
        ui.label("content");
    }
}

fn show(size: egui::Vec2, build: impl Fn() -> Frame, events: Vec<egui::Event>) -> FrameOutcome {
    let context = egui::Context::default();
    let mut outcome = FrameOutcome::default();
    for pass in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events: match pass {
                1 => events.clone(),
                _ => Vec::new(),
            },
            ..egui::RawInput::default()
        };
        let mut bands = Bands::default();
        let _ = context.run_ui(input, |ui| {
            outcome = build().show(ui, &mut bands);
        });
    }
    outcome
}

fn frame() -> Frame {
    Frame::new(egui::Id::new("test"))
        .toolbar(true)
        .left_sidebar(true)
        .right_sidebar(true)
}

mod a_narrow_frame_floats_its_sidebars_and_keeps_the_content_full_width;
mod escape_only_leaves_a_frame_that_has_a_trail;
mod the_bands_tile_the_frame_without_overlapping;
mod the_content_band_survives_handing_the_chrome_over;
