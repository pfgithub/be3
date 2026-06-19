mod binary_addition;
mod component_files;
mod editor;
mod renderer;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use component_files::{ChallengeProgress, ChallengeSolutionFile, ComponentFile, ComponentFiles};
use editor::LogicEditor;
use eframe::egui;
use logicgame::challenges;
use logicgame::grid::ComponentFileRef;
use renderer::GridRenderer;
use uuid::Uuid;

use crate::binary_addition::BinaryAdditionQuiz;
use crate::component_files::ComponentFileSource;

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
    component_files_list: Vec<ComponentFile>,
    challenge_solutions: BTreeMap<challenges::ChallengeId, Vec<ChallengeSolutionFile>>,
    challenge_progress: ChallengeProgress,
    binary_addition: BinaryAdditionQuiz,
    active_file: Option<ActiveFile>,
    persistence_error: Option<String>,
    new_component_open: bool,
    new_component_name: String,
    new_component_error: Option<String>,
    rename: Option<RenameState>,
    observed_revision: u64,
    saved_revision: u64,
    save_due: Option<Instant>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ActiveFile {
    Component {
        id: Uuid,
    },
    ChallengeSolution {
        challenge: challenges::ChallengeId,
        id: Uuid,
        completed: bool,
    },
    SpecialChallenge {
        challenge: challenges::ChallengeId,
    },
}

impl ActiveFile {
    fn file_ref(&self) -> Option<ComponentFileRef> {
        match self {
            Self::Component { id } | Self::ChallengeSolution { id, .. } => {
                Some(ComponentFileRef { id: *id })
            }
            Self::SpecialChallenge { .. } => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RenameState {
    path: ComponentFileSource,
    file: ComponentFileRef,
    name: String,
    error: Option<String>,
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
            component_files_list: Vec::new(),
            challenge_solutions: BTreeMap::new(),
            challenge_progress: ChallengeProgress::default(),
            binary_addition: BinaryAdditionQuiz::default(),
            active_file: None,
            persistence_error: None,
            new_component_open: false,
            new_component_name: String::new(),
            new_component_error: None,
            rename: None,
            observed_revision: 0,
            saved_revision: 0,
            save_due: None,
        };
        if game.component_files.is_none() {
            game.persistence_error =
                Some("The operating system application-data directory is unavailable".to_owned());
        } else {
            game.refresh_files();
            game.refresh_hotbar();
        }
        game
    }

    /// Loads the editor's custom hotbar slots from the persisted, already-compiled
    /// entries. Components are compiled only when first added, never on load.
    fn refresh_hotbar(&mut self) {
        let Some(files) = &self.component_files else {
            return;
        };
        let entries = match files.load_hotbar() {
            Ok(entries) => entries,
            Err(error) => {
                self.persistence_error = Some(error.to_string());
                return;
            }
        };
        let slots = entries
            .into_iter()
            .filter_map(|(name, kind)| match &kind {
                logicgame::grid::ComponentKind::Subcomponent { source_file_id, .. } => {
                    Some((name, *source_file_id, kind))
                }
                _ => None,
            })
            .collect();
        self.editor.set_custom_hotbar(slots);
    }

    fn refresh_files(&mut self) {
        let Some(files) = &self.component_files else {
            return;
        };
        match files.list() {
            Ok(files) => self.component_files_list = files,
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
        self.challenge_solutions.clear();
        for challenge_id in challenges::CHALLENGES {
            match files.list_challenge_solutions(challenge_id) {
                Ok(solutions) => {
                    self.challenge_solutions.insert(challenge_id, solutions);
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
        let mut requested_rename = None;
        let mut requested_delete_component = None;
        let mut requested_delete_solution = None;
        let mut requested_add_hotbar: Option<(ComponentFileRef, String)> = None;
        egui::Window::new("Components")
            .default_pos([700.0, 16.0])
            .default_width(220.0)
            .hscroll(true)
            .vscroll(true)
            .show(context, |ui| {
                ui.strong("Challenges");
                for challenge_id in challenges::CHALLENGES {
                    let active = self.editor.active_challenge_id() == Some(challenge_id)
                        || self.active_special_challenge() == Some(challenge_id);
                    let passed = self.challenge_progress.is_passed(challenge_id);
                    ui.horizontal(|ui| {
                        let label = if passed {
                            format!("{} pass", challenge_id.name())
                        } else {
                            challenge_id.name().to_owned()
                        };
                        if ui.selectable_label(active, label).clicked() {
                            if challenge_id.is_special() {
                                requested_solution = None;
                                requested_new_solution = Some(challenge_id);
                            } else if self
                                .challenge_solutions
                                .get(&challenge_id)
                                .is_none_or(Vec::is_empty)
                            {
                                requested_new_solution = Some(challenge_id);
                            }
                        }
                        if !challenge_id.is_special() && ui.button("New Solution").clicked() {
                            requested_new_solution = Some(challenge_id);
                        }
                        if challenge_id.is_special() {
                            ui.weak("quiz");
                        }
                    });
                    if challenge_id.is_special() {
                        continue;
                    }
                    for solution in self
                        .challenge_solutions
                        .get(&challenge_id)
                        .into_iter()
                        .flatten()
                    {
                        let file = ComponentFileRef { id: solution.id };
                        let active = self
                            .active_file
                            .as_ref()
                            .is_some_and(|active| active.file_ref() == Some(file));
                        ui.horizontal(|ui| {
                            ui.add_space(12.0);
                            let text = if solution.completed {
                                format!("{} pass", solution.name)
                            } else {
                                solution.name.clone()
                            };
                            let response = ui.selectable_label(active, text);
                            if response.clicked() {
                                requested_solution = Some((challenge_id, solution.id));
                            }
                            response.context_menu(|ui| {
                                if ui.button("Add to hotbar").clicked() {
                                    requested_add_hotbar =
                                        Some((file.clone(), solution.name.clone()));
                                    ui.close();
                                }
                                if ui.button("Rename").clicked() {
                                    requested_rename = Some((
                                        ComponentFileSource::ChallengeSolution {
                                            challenge: challenge_id,
                                        },
                                        file,
                                        solution.name.clone(),
                                    ));
                                    ui.close();
                                }
                                if ui.button("Delete").clicked() {
                                    requested_delete_solution = Some((challenge_id, solution.id));
                                    ui.close();
                                }
                            });
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
                if self.component_files_list.is_empty() {
                    ui.weak("No component files");
                } else {
                    for component in &self.component_files_list {
                        let file = ComponentFileRef { id: component.id };
                        let active = self
                            .active_file
                            .as_ref()
                            .is_some_and(|active| active.file_ref() == Some(file));
                        let response = ui.selectable_label(active, &component.name);
                        if response.clicked() {
                            requested_component = Some(component.id);
                        }
                        response.context_menu(|ui| {
                            if ui.button("Add to hotbar").clicked() {
                                requested_add_hotbar = Some((file, component.name.clone()));
                                ui.close();
                            }
                            if ui.button("Rename").clicked() {
                                requested_rename = Some((
                                    ComponentFileSource::Component,
                                    file,
                                    component.name.clone(),
                                ));
                                ui.close();
                            }
                            if ui.button("Delete").clicked() {
                                requested_delete_component = Some(component.id);
                                ui.close();
                            }
                        });
                    }
                }

                if let Some(error) = &self.persistence_error {
                    ui.separator();
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });

        if let Some((path, file, name)) = requested_rename {
            self.rename = Some(RenameState {
                path,
                file,
                name,
                error: None,
            });
        }
        if let Some(id) = requested_component {
            self.open_component(id);
        }
        if let Some((challenge, id)) = requested_solution {
            self.open_challenge_solution(challenge, id);
        }
        if let Some(id) = requested_new_solution {
            if id.is_special() {
                self.open_special_challenge(id);
            } else {
                self.create_challenge_solution(id);
            }
        }
        if let Some((challenge, id)) = requested_delete_solution {
            self.delete_challenge_solution(challenge, id);
        }
        if let Some(id) = requested_delete_component {
            self.delete_component(id);
        }
        if let Some((file, name)) = requested_add_hotbar {
            self.add_component_to_hotbar(file, name);
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

    fn show_rename_modal(&mut self, context: &egui::Context) {
        let Some(rename) = &mut self.rename else {
            return;
        };

        let mut submit = false;
        let mut cancel = false;
        let response = egui::Modal::new("rename-component-modal".into()).show(context, |ui| {
            ui.set_min_width(320.0);
            ui.heading("Rename");
            ui.label("Name");
            let name =
                ui.add(egui::TextEdit::singleline(&mut rename.name).desired_width(f32::INFINITY));
            name.request_focus();
            if let Some(error) = &rename.error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            ui.horizontal(|ui| {
                submit = ui.button("Rename").clicked()
                    || name.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                cancel = ui.button("Cancel").clicked();
            });
        });
        if response.should_close() || cancel {
            self.rename = None;
            return;
        }
        if submit {
            self.rename_file();
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
            Ok((id, grid)) => {
                self.editor.replace_grid(grid);
                self.active_file = Some(ActiveFile::Component { id });
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

    fn open_component(&mut self, id: Uuid) {
        let file = ComponentFileRef { id };
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == Some(file))
            || !self.force_save()
        {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.load_ref(&file) {
            Ok(grid) => {
                self.editor.replace_grid(grid);
                self.active_file = Some(ActiveFile::Component { id });
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn open_special_challenge(&mut self, challenge: challenges::ChallengeId) {
        if self.active_special_challenge() == Some(challenge) || !self.force_save() {
            return;
        }
        self.editor.replace_grid(logicgame::grid::LogicGrid::new());
        self.binary_addition = BinaryAdditionQuiz::default();
        self.active_file = Some(ActiveFile::SpecialChallenge { challenge });
        self.observed_revision = 0;
        self.saved_revision = 0;
        self.save_due = None;
        self.persistence_error = None;
    }

    fn active_special_challenge(&self) -> Option<challenges::ChallengeId> {
        match self.active_file.as_ref() {
            Some(ActiveFile::SpecialChallenge { challenge }) => Some(*challenge),
            _ => None,
        }
    }

    fn delete_component(&mut self, id: Uuid) {
        let file = ComponentFileRef { id };
        let deleting_active = self.active_file.as_ref().is_some_and(
            |active| matches!(active, ActiveFile::Component { id: active_id } if *active_id == id),
        );
        if deleting_active && !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.delete(&file) {
            Ok(()) => {
                if deleting_active {
                    self.active_file = None;
                    self.observed_revision = 0;
                    self.saved_revision = 0;
                    self.save_due = None;
                }
                self.persistence_error = None;
                self.refresh_files();
                self.refresh_hotbar();
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn create_challenge_solution(&mut self, id: challenges::ChallengeId) {
        if id.is_special() {
            self.open_special_challenge(id);
            return;
        }
        if !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.create_challenge_solution(id) {
            Ok((file_id, _name, grid)) => {
                self.editor.open_challenge_solution(id, grid);
                self.active_file = Some(ActiveFile::ChallengeSolution {
                    challenge: id,
                    id: file_id,
                    completed: false,
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

    fn open_challenge_solution(&mut self, id: challenges::ChallengeId, file_id: Uuid) {
        let file = ComponentFileRef { id: file_id };
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == Some(file))
            || !self.force_save()
        {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.load_ref(&file) {
            Ok(grid) => {
                let completed = self
                    .challenge_solutions
                    .get(&id)
                    .and_then(|solutions| {
                        solutions.iter().find_map(|solution| {
                            (solution.id == file_id).then_some(solution.completed)
                        })
                    })
                    .unwrap_or(false);
                self.editor.open_challenge_solution(id, grid);
                self.active_file = Some(ActiveFile::ChallengeSolution {
                    challenge: id,
                    id: file_id,
                    completed,
                });
                self.observed_revision = self.editor.grid().revision();
                self.saved_revision = self.editor.grid().revision();
                self.save_due = None;
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn delete_challenge_solution(&mut self, challenge: challenges::ChallengeId, id: Uuid) {
        let deleting_active = self.active_file.as_ref().is_some_and(|active| {
            matches!(
                active,
                ActiveFile::ChallengeSolution {
                    challenge: active_challenge,
                    id: active_id,
                    ..
                } if *active_challenge == challenge && *active_id == id
            )
        });
        if deleting_active && !self.force_save() {
            return;
        }
        let Some(files) = &self.component_files else {
            return;
        };
        match files.delete_challenge_solution(challenge, id) {
            Ok(()) => {
                if deleting_active {
                    self.active_file = None;
                    self.observed_revision = 0;
                    self.saved_revision = 0;
                    self.save_due = None;
                }
                self.persistence_error = None;
                self.refresh_files();
                self.refresh_hotbar();
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }

    fn mark_active_challenge_passed(&mut self) {
        let Some(ActiveFile::ChallengeSolution {
            challenge,
            id,
            completed,
        }) = &mut self.active_file
        else {
            return;
        };
        let challenge = *challenge;
        let id = *id;
        *completed = true;
        self.set_cached_solution_completed(challenge, id, true);
        self.challenge_progress.passed.insert(challenge, true);
        self.observed_revision = self.editor.grid().revision();
        self.saved_revision = self.saved_revision.wrapping_sub(1);
        self.save_due = Some(Instant::now());
        self.force_save();
    }

    fn mark_special_challenge_passed(&mut self, challenge: challenges::ChallengeId) {
        self.challenge_progress.passed.insert(challenge, true);
        if let Some(files) = &self.component_files {
            if let Err(error) = files.save_progress(&self.challenge_progress) {
                self.persistence_error = Some(error.to_string());
                return;
            }
        }
        self.persistence_error = None;
        self.refresh_files();
    }

    fn set_cached_solution_completed(
        &mut self,
        challenge: challenges::ChallengeId,
        id: Uuid,
        completed: bool,
    ) {
        if let Some(solutions) = self.challenge_solutions.get_mut(&challenge) {
            if let Some(solution) = solutions.iter_mut().find(|solution| solution.id == id) {
                solution.completed = completed;
            }
        }
    }

    fn observe_changes(&mut self) {
        let revision = self.editor.grid().revision();
        if self.active_file.is_some() && revision != self.observed_revision {
            self.observed_revision = revision;
            let changed_solution = if let Some(ActiveFile::ChallengeSolution {
                challenge,
                id,
                completed,
            }) = &mut self.active_file
            {
                *completed = false;
                Some((*challenge, *id))
            } else {
                None
            };
            if let Some((challenge, id)) = changed_solution {
                self.set_cached_solution_completed(challenge, id, false);
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
            ActiveFile::Component { id } => {
                files.save(&ComponentFileRef { id }, self.editor.grid())
            }
            ActiveFile::ChallengeSolution {
                challenge,
                id,
                completed,
            } => {
                let result =
                    files.save_challenge_solution(challenge, id, self.editor.grid(), completed);
                if result.is_ok() {
                    if let Err(error) = files.save_progress(&self.challenge_progress) {
                        return self.handle_save_error(error);
                    }
                }
                result
            }
            ActiveFile::SpecialChallenge { .. } => {
                self.save_due = None;
                return true;
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

    fn rename_file(&mut self) {
        if !self.force_save() {
            return;
        }
        let Some(rename) = self.rename.clone() else {
            return;
        };
        let Some(files) = &self.component_files else {
            return;
        };
        match files.rename(&rename.path, &rename.file, &rename.name) {
            Ok(()) => {
                self.persistence_error = None;
                self.rename = None;
                self.refresh_files();
            }
            Err(error) => {
                if let Some(state) = &mut self.rename {
                    state.error = Some(error.to_string());
                }
            }
        }
    }

    fn handle_save_error(&mut self, error: component_files::ComponentFileError) -> bool {
        self.persistence_error = Some(error.to_string());
        self.save_due = Some(Instant::now() + AUTOSAVE_DELAY);
        false
    }

    fn add_component_to_hotbar(&mut self, file: ComponentFileRef, name: String) {
        if self
            .active_file
            .as_ref()
            .is_some_and(|active| active.file_ref() == Some(file))
        {
            self.persistence_error =
                Some("A component cannot contain the file currently being edited".to_owned());
            return;
        }
        let Some(files) = self.component_files.clone() else {
            return;
        };
        match files.compile_subcomponent(&file, &name) {
            Ok(kind) => {
                if let Err(error) = files.add_hotbar(&name, &kind) {
                    self.persistence_error = Some(error.to_string());
                    return;
                }
                self.editor.add_custom_hotbar_slot(name, file, kind);
                self.persistence_error = None;
            }
            Err(error) => self.persistence_error = Some(error.to_string()),
        }
    }
}

impl eframe::App for LogicGame {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        match self.active_file.clone() {
            Some(ActiveFile::SpecialChallenge { challenge }) => {
                match challenge {
                    challenges::ChallengeId::BinaryAddition => self.binary_addition.ui(ui),
                    _ => {}
                }
                if self.binary_addition.take_passed() {
                    self.mark_special_challenge_passed(challenge);
                }
            }
            Some(_) => {
                self.editor.ui(ui);
                self.observe_changes();
                if self.editor.take_challenge_passed() {
                    self.mark_active_challenge_passed();
                }
                self.autosave();
            }
            None => {
                egui::Frame::central_panel(ui.style()).show(ui, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.weak("Create or select a component to begin editing");
                    });
                });
            }
        }

        let context = ui.ctx().clone();
        self.show_components(&context);
        self.show_new_component_modal(&context);
        self.show_rename_modal(&context);
        context.request_repaint_after(AUTOSAVE_DELAY);
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.force_save();
    }
}
