mod component_files;
mod editor;
mod renderer;

use std::time::{Duration, Instant};

use component_files::{ComponentFileDrag, ComponentFiles};
use editor::LogicEditor;
use eframe::egui;
use renderer::GridRenderer;

const APP_ID: &str = "Logic Game";
const AUTOSAVE_DELAY: Duration = Duration::from_millis(300);

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_app_id(APP_ID)
            .with_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_ID,
        options,
        Box::new(|creation_context| Ok(Box::new(LogicGame::new(creation_context)))),
    )
}

struct LogicGame {
    editor: LogicEditor,
    component_files: Option<ComponentFiles>,
    component_names: Vec<String>,
    active_component: Option<String>,
    persistence_error: Option<String>,
    new_component_open: bool,
    new_component_name: String,
    new_component_error: Option<String>,
    observed_revision: u64,
    saved_revision: u64,
    save_due: Option<Instant>,
}

impl LogicGame {
    fn new(creation_context: &eframe::CreationContext<'_>) -> Self {
        let render_state = creation_context
            .wgpu_render_state
            .as_ref()
            .expect("logicgame requires the wgpu renderer");
        render_state
            .renderer
            .write()
            .callback_resources
            .insert(GridRenderer::new(
                &render_state.device,
                render_state.target_format,
            ));

        let component_files =
            eframe::storage_dir(APP_ID).map(|root| ComponentFiles::new(root.join("components")));
        let mut game = Self {
            editor: LogicEditor::default(),
            component_files,
            component_names: Vec::new(),
            active_component: None,
            persistence_error: None,
            new_component_open: false,
            new_component_name: String::new(),
            new_component_error: None,
            observed_revision: 0,
            saved_revision: 0,
            save_due: None,
        };
        if game.component_files.is_none() {
            game.persistence_error =
                Some("The operating system application-data directory is unavailable".to_owned());
        } else {
            game.refresh_component_names();
        }
        game
    }

    fn refresh_component_names(&mut self) {
        let Some(files) = &self.component_files else {
            return;
        };
        match files.list() {
            Ok(names) => self.component_names = names,
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn show_components(&mut self, context: &egui::Context) {
        let mut requested_component = None;
        egui::Window::new("Components")
            .default_pos([700.0, 16.0])
            .default_width(220.0)
            .show(context, |ui| {
                if ui
                    .add_enabled(self.component_files.is_some(), egui::Button::new("New"))
                    .clicked()
                {
                    self.new_component_open = true;
                    self.new_component_name.clear();
                    self.new_component_error = None;
                }

                ui.separator();
                if self.component_names.is_empty() {
                    ui.weak("No component files");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(180.0)
                        .show(ui, |ui| {
                            for name in &self.component_names {
                                let active = self.active_component.as_ref() == Some(name);
                                if active {
                                    if ui.selectable_label(true, name).clicked() {
                                        requested_component = Some(name.clone());
                                    }
                                } else {
                                    let response = ui
                                        .selectable_label(false, name)
                                        .interact(egui::Sense::click_and_drag());
                                    response.dnd_set_drag_payload(ComponentFileDrag {
                                        name: name.clone(),
                                    });
                                    if response.clicked() {
                                        requested_component = Some(name.clone());
                                    }
                                }
                            }
                        });
                }

                if let Some(error) = &self.persistence_error {
                    ui.separator();
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });

        if let Some(name) = requested_component {
            self.open_component(&name);
        }
    }

    fn show_new_component_modal(&mut self, context: &egui::Context) {
        if !self.new_component_open {
            return;
        }

        let mut create = false;
        let response = egui::Modal::new("new-component-modal".into()).show(context, |ui| {
            ui.set_min_width(320.0);
            ui.heading("New component");
            ui.label("Name");
            let name = ui.add(
                egui::TextEdit::singleline(&mut self.new_component_name)
                    .desired_width(f32::INFINITY),
            );
            name.request_focus();
            if let Some(error) = &self.new_component_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                create = ui.button("Create").clicked()
                    || name.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                if ui.button("Cancel").clicked() {
                    self.new_component_open = false;
                }
            });
        });
        if response.should_close() {
            self.new_component_open = false;
        }
        if create {
            self.create_component();
        }
    }

    fn create_component(&mut self) {
        if !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.create(&self.new_component_name) {
            Ok(grid) => {
                let name = self.new_component_name.clone();
                self.editor.replace_grid(grid);
                self.active_component = Some(name);
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
                self.new_component_open = false;
                self.new_component_error = None;
                self.refresh_component_names();
            }
            Err(error) => self.new_component_error = Some(error.to_string()),
        }
    }

    fn open_component(&mut self, name: &str) {
        if self.active_component.as_deref() == Some(name) || !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.load(name) {
            Ok(grid) => {
                self.editor.replace_grid(grid);
                self.active_component = Some(name.to_owned());
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn observe_changes(&mut self) {
        let revision = self.editor.grid().revision();
        if self.active_component.is_some() && revision != self.observed_revision {
            self.observed_revision = revision;
            self.save_due = Some(Instant::now() + AUTOSAVE_DELAY);
        }
    }

    fn autosave(&mut self) {
        if self
            .save_due
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.force_save();
        }
    }

    fn force_save(&mut self) -> bool {
        let Some(name) = self.active_component.as_deref() else {
            return true;
        };
        if self.editor.grid().revision() == self.saved_revision {
            self.save_due = None;
            return true;
        }
        let Some(files) = &self.component_files else {
            return false;
        };
        match files.save(name, self.editor.grid()) {
            Ok(()) => {
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
                true
            }
            Err(error) => {
                self.persistence_error = Some(error.to_string());
                self.save_due = Some(Instant::now() + AUTOSAVE_DELAY);
                false
            }
        }
    }

    fn drop_component_file(&mut self, name: &str, position: logicgame::grid::Point) {
        if self.active_component.as_deref() == Some(name) {
            self.persistence_error =
                Some("A component cannot contain the file currently being edited".to_owned());
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.compile_subcomponent(name) {
            Ok(kind) => {
                self.editor.insert_subcomponent(position, kind);
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }
}

impl eframe::App for LogicGame {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.active_component.is_some() {
            if let Some(dropped) = self.editor.ui(ui) {
                self.drop_component_file(&dropped.name, dropped.position);
            }
            self.observe_changes();
            self.autosave();
        } else {
            egui::Frame::central_panel(ui.style()).show(ui, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.weak("Create or select a component to begin editing");
                });
            });
        }

        let context = ui.ctx().clone();
        self.show_components(&context);
        self.show_new_component_modal(&context);
        context.request_repaint_after(AUTOSAVE_DELAY);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.force_save();
    }
}
