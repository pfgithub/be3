use block_editor_plugin::App;
use egui_kittest::kittest::{by, Queryable as _};
use egui_kittest::{Harness, Node};

use crate::snapshot;
use crate::textures::Textures;

const SIDEBAR_WIDTH: f32 = 160.0;
const SEPARATOR_WIDTH: f32 = 12.0;

pub struct EditorTest<'a, A: App> {
    harness: Harness<'a, A>,
    textures: Textures,
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
        Self { harness, textures }
    }

    pub fn run(&mut self) {
        self.harness.run();
    }

    pub fn find<'t>(&'t self, test_id: &'t str) -> Node<'t> {
        self.harness
            .get(by().predicate(move |node| node.author_id() == Some(test_id)))
    }

    pub fn app(&mut self) -> &mut A {
        self.harness.state_mut()
    }

    pub fn snapshot(&mut self, name: &str) {
        snapshot::assert_snapshot(
            name,
            &paint_snapshot::capture(
                &self.harness.ctx,
                self.harness.output(),
                &self.textures.store(),
            )
            .expect("the painting could not be captured"),
        );
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
