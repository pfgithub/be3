use block_editor_plugin::App;
use egui_kittest::kittest::{by, Queryable as _};
use egui_kittest::{Harness, Node};
use paint_snapshot::Snapshot;

use crate::snapshot;
use crate::textures::Textures;

const SIDEBAR_WIDTH: f32 = 160.0;
const SEPARATOR_WIDTH: f32 = 12.0;

pub struct EditorTest<'a, A: App> {
    harness: Harness<'a, A>,
    textures: Textures,
    recording: Option<Snapshot>,
}

impl<A: App> EditorTest<'_, A> {
    pub fn new(app: A) -> Self {
        let textures = Textures::default();
        let harness = Harness::builder()
            .renderer(textures.clone())
            .build_ui_state(
                |ui, app: &mut A| {
                    ui.vertical(|ui| {
                        app.toolbar_ui(ui);
                        ui.separator();
                        ui.horizontal_top(|ui| {
                            let height = ui.available_height();
                            column(ui, SIDEBAR_WIDTH, height, |ui| app.left_sidebar_ui(ui));
                            ui.separator();
                            let width = ui.available_width() - SIDEBAR_WIDTH - SEPARATOR_WIDTH;
                            column(ui, width, height, |ui| app.ui(ui));
                            ui.separator();
                            column(ui, SIDEBAR_WIDTH, height, |ui| app.right_sidebar_ui(ui));
                        });
                    });
                },
                app,
            );
        Self {
            harness,
            textures,
            recording: None,
        }
    }

    pub fn run(&mut self) {
        self.harness.run();
    }

    pub fn step(&mut self) {
        self.harness.step();
    }

    pub fn find<'t>(&'t self, test_id: &'t str) -> Node<'t> {
        self.harness
            .get(by().predicate(move |node| node.author_id() == Some(test_id)))
    }

    pub fn app(&mut self) -> &mut A {
        self.harness.state_mut()
    }

    pub fn record(&mut self) {
        let frame = self.painted();
        match &mut self.recording {
            Some(recording) => recording.append(frame),
            None => self.recording = Some(frame),
        }
    }

    pub fn snapshot(&mut self, name: &str) {
        let painting = match self.recording.take() {
            Some(recording) => recording,
            None => self.painted(),
        };
        snapshot::assert_snapshot(name, &painting);
    }

    fn painted(&mut self) -> Snapshot {
        paint_snapshot::capture(
            &self.harness.ctx,
            self.harness.output(),
            &self.textures.store(),
        )
        .expect("the painting could not be captured")
    }
}

fn column(ui: &mut egui::Ui, width: f32, height: f32, contents: impl FnOnce(&mut egui::Ui)) {
    let size = egui::vec2(width.max(0.0), height.max(0.0));
    let layout = egui::Layout::top_down(egui::Align::Min);
    ui.allocate_ui_with_layout(size, layout, |ui| {
        ui.set_min_size(size);
        contents(ui);
    });
}
