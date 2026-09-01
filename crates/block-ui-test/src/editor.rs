use block_editor_plugin::{App, EditorHost, ViewChange};
use egui_kittest::kittest::{by, Queryable as _};
use egui_kittest::{Harness, Node};
use paint_snapshot::Snapshot;

use crate::snapshot;
use crate::textures::Textures;

const SIDEBAR_WIDTH: f32 = 160.0;
const SEPARATOR_WIDTH: f32 = 12.0;
const MINIMUM_ZOOM: f32 = 1.0 / 64.0;
const MAXIMUM_ZOOM: f32 = 32.0;

pub struct EditorTest<'a, A: App> {
    harness: Harness<'a, A>,
    textures: Textures,
    recording: Option<Snapshot>,
}

impl<A: App> EditorTest<'_, A> {
    pub fn new(app: A) -> Self {
        Self::build(app, None)
    }

    pub fn viewport(app: A, host: EditorHost) -> Self {
        Self::build(app, Some(Viewport::new(host)))
    }

    fn build(app: A, mut viewport: Option<Viewport>) -> Self {
        let textures = Textures::default();
        let mut installed = false;
        let mut harness = Harness::builder()
            .renderer(textures.clone())
            .build_ui_state(
                move |ui, app: &mut A| {
                    if !installed {
                        installed = true;
                        block_editor_plugin::egui_material_icons::initialize(ui.ctx());
                        ui.ctx().request_discard("the icon font was just installed");
                        return;
                    }
                    ui.vertical(|ui| {
                        app.toolbar_ui(ui);
                        ui.separator();
                        ui.horizontal_top(|ui| {
                            let height = ui.available_height();
                            column(ui, SIDEBAR_WIDTH, height, |ui| app.left_sidebar_ui(ui));
                            ui.separator();
                            let width = ui.available_width() - SIDEBAR_WIDTH - SEPARATOR_WIDTH;
                            column(ui, width, height, |ui| match &mut viewport {
                                Some(viewport) => {
                                    let region = ui.available_rect_before_wrap();
                                    viewport.place(region, app.intrinsic_size());
                                    app.ui(ui);
                                    viewport.settle(region);
                                }
                                None => app.ui(ui),
                            });
                            ui.separator();
                            column(ui, SIDEBAR_WIDTH, height, |ui| app.right_sidebar_ui(ui));
                        });
                    });
                },
                app,
            );
        harness.step();
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

    pub fn key_press(&self, key: egui::Key) {
        self.harness.key_press(key);
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

struct Viewport {
    host: EditorHost,
    zoom: f32,
    pan: egui::Vec2,
    fitting: bool,
}

impl Viewport {
    fn new(host: EditorHost) -> Self {
        Self {
            host,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            fitting: true,
        }
    }

    fn place(&mut self, region: egui::Rect, intrinsic: Option<egui::Vec2>) {
        let content = intrinsic
            .unwrap_or(egui::Vec2::ZERO)
            .max(region.size())
            .max(egui::Vec2::splat(1.0));
        if self.fitting {
            self.zoom = (region.width() / content.x)
                .min(region.height() / content.y)
                .min(1.0)
                .clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
            self.pan = egui::Vec2::ZERO;
        }
        let view = egui::Rect::from_center_size(region.center() + self.pan, content * self.zoom);
        self.host.set_view(view);
    }

    fn settle(&mut self, region: egui::Rect) {
        for change in self.host.take_view_changes() {
            self.fitting = false;
            match change {
                ViewChange::Pan { x, y } => self.pan += egui::vec2(x, y),
                ViewChange::Zoom { factor, anchor } => {
                    let zoom = (self.zoom * factor).clamp(MINIMUM_ZOOM, MAXIMUM_ZOOM);
                    let anchor =
                        anchor.map_or(region.center(), |(x, y)| egui::pos2(x, y)) - region.center();
                    self.pan = anchor - (anchor - self.pan) * (zoom / self.zoom);
                    self.zoom = zoom;
                }
                ViewChange::Fit => self.fitting = true,
            }
        }
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
