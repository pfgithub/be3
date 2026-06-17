mod component_files;
mod editor;
mod renderer;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use component_files::{
    ChallengeProgress, ChallengeSolutionFile, ComponentFileDrag, ComponentFileRef, ComponentFiles,
};
use editor::LogicEditor;
use eframe::egui;
use logicgame::challenges;
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
    challenge_solutions: BTreeMap<challenges::ChallengeId, Vec<ChallengeSolutionFile>>,
    challenge_progress: ChallengeProgress,
    active_file: Option<ActiveFile>,
    persistence_error: Option<String>,
    new_component_open: bool,
    new_component_name: String,
    new_component_error: Option<String>,
    observed_revision: u64,
    saved_revision: u64,
    save_due: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ActiveFile {
    Component(String),
    ChallengeSolution {
        challenge: challenges::ChallengeId,
        name: String,
        passing: bool,
    },
}

impl ActiveFile {
    fn file_ref(&self) -> ComponentFileRef {
        match self {
            Self::Component(name) => ComponentFileRef::Component(name.clone()),
            Self::ChallengeSolution {
                challenge, name, ..
            } => ComponentFileRef::ChallengeSolution {
                challenge: *challenge,
                name: name.clone(),
            },
        }
    }
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
        let mut editor = LogicEditor::default();
        editor.set_component_files(component_files.clone());
        let mut game = Self {
            editor,
            component_files,
            component_names: Vec::new(),
            challenge_solutions: BTreeMap::new(),
            challenge_progress: ChallengeProgress::default(),
            active_file: None,
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
            game.refresh_files();
        }
        game
    }

    fn refresh_files(&mut self) {
        let Some(files) = &self.component_files else {
            return;
        };
        match files.list() {
            Ok(names) => self.component_names = names,
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
        self.challenge_solutions.clear();
        for challenge in challenges::CHALLENGES {
            match files.list_challenge_solutions(challenge.id) {
                Ok(solutions) => {
                    self.challenge_solutions.insert(challenge.id, solutions);
                }
                Err(error) => self.persistence_error = Some(error.to_string()),
            }
        }
        match files.load_progress() {
            Ok(progress) => self.challenge_progress = progress,
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn show_components(&mut self, context: &egui::Context) {
        let mut requested_component = None;
        let mut requested_solution = None;
        let mut requested_new_solution = None;
        egui::Window::new("Components")
            .default_pos([700.0, 16.0])
            .default_width(220.0)
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                ui.strong("Challenges");
                for challenge in challenges::CHALLENGES {
                    let active = self.editor.active_challenge_id() == Some(challenge.id);
                    let passed = self.challenge_progress.is_passed(challenge.id);
                    ui.horizontal(|ui| {
                        let label = if passed {
                            format!("{} pass", challenge.name)
                        } else {
                            challenge.name.to_owned()
                        };
                        if ui.selectable_label(active, label).clicked()
                            && self
                                .challenge_solutions
                                .get(&challenge.id)
                                .is_none_or(Vec::is_empty)
                        {
                            requested_new_solution = Some(challenge.id);
                        }
                        if ui.button("New Solution").clicked() {
                            requested_new_solution = Some(challenge.id);
                        }
                    });
                    for solution in self
                        .challenge_solutions
                        .get(&challenge.id)
                        .into_iter()
                        .flatten()
                    {
                        let file = ComponentFileRef::ChallengeSolution {
                            challenge: challenge.id,
                            name: solution.name.clone(),
                        };
                        let active = self
                            .active_file
                            .as_ref()
                            .is_some_and(|active| active.file_ref() == file);
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            let text = if solution.passing {
                                format!("{} pass", solution.name)
                            } else {
                                solution.name.clone()
                            };
                            let response = ui
                                .selectable_label(active, text)
                                .interact(egui::Sense::click_and_drag());
                            if !active {
                                response
                                    .dnd_set_drag_payload(ComponentFileDrag { file: file.clone() });
                            }
                            if response.clicked() {
                                requested_solution = Some((challenge.id, solution.name.clone()));
                            }
                        });
                    }
                }
                ui.separator();
                ui.strong("Components");
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
                    for name in &self.component_names {
                        let file = ComponentFileRef::Component(name.clone());
                        let active = self
                            .active_file
                            .as_ref()
                            .is_some_and(|active| active.file_ref() == file);
                        if active {
                            if ui.selectable_label(true, name).clicked() {
                                requested_component = Some(name.clone());
                            }
                        } else {
                            let response = ui
                                .selectable_label(false, name)
                                .interact(egui::Sense::click_and_drag());
                            response.dnd_set_drag_payload(ComponentFileDrag { file });
                            if response.clicked() {
                                requested_component = Some(name.clone());
                            }
                        }
                    }
                }

                if let Some(error) = &self.persistence_error {
                    ui.separator();
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });

        if let Some(name) = requested_component {
            self.open_component(&name);
        }
        if let Some((id, name)) = requested_solution {
            self.open_challenge_solution(id, &name);
        }
        if let Some(id) = requested_new_solution {
            self.create_challenge_solution(id);
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
                self.active_file = Some(ActiveFile::Component(name));
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
                self.new_component_open = false;
                self.new_component_error = None;
                self.refresh_files();
            }
            Err(error) => self.new_component_error = Some(error.to_string()),
        }
    }

    fn open_component(&mut self, name: &str) {
        let file = ComponentFileRef::Component(name.to_owned());
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == file)
            || !self.force_save()
        {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.load(name) {
            Ok(grid) => {
                self.editor.replace_grid(grid);
                self.active_file = Some(ActiveFile::Component(name.to_owned()));
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn create_challenge_solution(&mut self, id: challenges::ChallengeId) {
        if !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.create_challenge_solution(id) {
            Ok((name, grid)) => {
                self.editor.open_challenge_solution(id, grid);
                self.active_file = Some(ActiveFile::ChallengeSolution {
                    challenge: id,
                    name,
                    passing: false,
                });
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
                self.refresh_files();
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn open_challenge_solution(&mut self, id: challenges::ChallengeId, name: &str) {
        let file = ComponentFileRef::ChallengeSolution {
            challenge: id,
            name: name.to_owned(),
        };
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == file)
            || !self.force_save()
        {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.load_ref(&file) {
            Ok(grid) => {
                let passing = self
                    .challenge_solutions
                    .get(&id)
                    .and_then(|solutions| {
                        solutions.iter().find_map(|solution| {
                            (solution.name == name).then_some(solution.passing)
                        })
                    })
                    .unwrap_or(false);
                self.editor.open_challenge_solution(id, grid);
                self.active_file = Some(ActiveFile::ChallengeSolution {
                    challenge: id,
                    name: name.to_owned(),
                    passing,
                });
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn mark_active_challenge_passed(&mut self) {
        let Some(ActiveFile::ChallengeSolution {
            challenge,
            name,
            passing,
        }) = &mut self.active_file
        else {
            return;
        };
        let challenge = *challenge;
        let name = name.clone();
        *passing = true;
        self.set_cached_solution_passing(challenge, &name, true);
        self.challenge_progress.passed.insert(challenge, true);
        self.observed_revision = self.editor.grid().revision();
        self.saved_revision = self.saved_revision.wrapping_sub(1);
        self.save_due = Some(Instant::now());
        self.force_save();
    }

    fn set_cached_solution_passing(
        &mut self,
        challenge: challenges::ChallengeId,
        name: &str,
        passing: bool,
    ) {
        if let Some(solutions) = self.challenge_solutions.get_mut(&challenge) {
            if let Some(solution) = solutions.iter_mut().find(|solution| solution.name == name) {
                solution.passing = passing;
            }
        }
    }

    fn observe_changes(&mut self) {
        let revision = self.editor.grid().revision();
        if self.active_file.is_some() && revision != self.observed_revision {
            self.observed_revision = revision;
            let changed_solution = if let Some(ActiveFile::ChallengeSolution {
                challenge,
                name,
                passing,
            }) = &mut self.active_file
            {
                *passing = false;
                Some((*challenge, name.clone()))
            } else {
                None
            };
            if let Some((challenge, name)) = changed_solution {
                self.set_cached_solution_passing(challenge, &name, false);
            };
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
        let Some(active) = self.active_file.clone() else {
            return true;
        };
        if self.editor.grid().revision() == self.saved_revision {
            self.save_due = None;
            return true;
        }
        let Some(files) = &self.component_files else {
            return false;
        };
        let result = match active {
            ActiveFile::Component(name) => files.save(&name, self.editor.grid()),
            ActiveFile::ChallengeSolution {
                challenge,
                name,
                passing,
            } => {
                let result =
                    files.save_challenge_solution(challenge, &name, self.editor.grid(), passing);
                if result.is_ok() {
                    if let Err(error) = files.save_progress(&self.challenge_progress) {
                        return self.handle_save_error(error);
                    }
                }
                result
            }
        };
        match result {
            Ok(()) => {
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
                self.refresh_files();
                true
            }
            Err(error) => self.handle_save_error(error),
        }
    }

    fn handle_save_error(&mut self, error: component_files::ComponentFileError) -> bool {
        self.persistence_error = Some(error.to_string());
        self.save_due = Some(Instant::now() + AUTOSAVE_DELAY);
        false
    }

    fn drop_component_file(&mut self, file: &ComponentFileRef, position: logicgame::grid::Point) {
        if self.editor.active_challenge_id().is_some() {
            self.persistence_error =
                Some("Subcomponents are not available in challenges".to_owned());
            return;
        }
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == *file)
        {
            self.persistence_error =
                Some("A component cannot contain the file currently being edited".to_owned());
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.compile_subcomponent(file) {
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
        if self.active_file.is_some() || self.editor.active_challenge_id().is_some() {
            if let Some(dropped) = self.editor.ui(ui) {
                self.drop_component_file(&dropped.file, dropped.position);
            }
            if self.editor.take_challenge_passed() {
                self.mark_active_challenge_passed();
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
