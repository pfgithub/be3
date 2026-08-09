mod github;
mod http;

#[cfg(target_os = "android")]
mod install;

use std::cell::RefCell;

use eframe::egui;

use github::WorkflowRun;

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

#[derive(Default)]
struct State {
    open: bool,
    runs_fetch: Option<http::Fetch>,
    runs: Option<Result<Vec<WorkflowRun>, String>>,
    #[cfg(target_os = "android")]
    install: Option<install::Install>,
}

impl State {
    fn start_fetch(&mut self) {
        self.runs = None;
        self.runs_fetch = Some(http::Fetch::get(github::runs_url(), github::api_headers()));
    }
}

/// Opens the version-switching debug window, fetching the workflow run list
/// if it has not been fetched yet.
pub(crate) fn open() {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.open = true;
        if state.runs.is_none() && state.runs_fetch.is_none() {
            state.start_fetch();
        }
    });
}

/// Draws the version-switching debug window, if open.
pub(crate) fn show(ctx: &egui::Context) {
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        if !state.open {
            return;
        }

        if let Some(fetch) = &state.runs_fetch {
            match fetch.poll() {
                Some(result) => {
                    state.runs = Some(result.and_then(|body| github::parse_runs(&body)));
                    state.runs_fetch = None;
                }
                None => ctx.request_repaint(),
            }
        }
        #[cfg(target_os = "android")]
        if let Some(install) = &mut state.install {
            if !install.finished() {
                install.poll();
                ctx.request_repaint();
            }
        }

        let mut open = state.open;
        egui::Window::new("App Version")
            .open(&mut open)
            .default_size([640.0, 480.0])
            .show(ctx, |ui| show_contents(ui, &mut state));
        state.open = open;
    });
}

fn show_contents(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.label("Running commit:");
        ui.add(egui::Label::new(egui::RichText::new(crate::COMMIT).monospace()).selectable(true));
        if ui.button("Refresh").clicked() {
            state.start_fetch();
        }
    });
    ui.separator();

    let runs = state.runs.clone();
    match runs {
        None => {
            ui.spinner();
        }
        Some(Err(error)) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        Some(Ok(runs)) => {
            if runs.is_empty() {
                ui.weak("No workflow runs found.");
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for run in &runs {
                    show_run(ui, run, state);
                }
            });
        }
    }
}

fn show_run(ui: &mut egui::Ui, run: &WorkflowRun, state: &mut State) {
    let current = run.head_sha == crate::COMMIT;
    ui.group(|ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong(format!("#{}", run.run_number));
            ui.label(&run.head_branch);
            ui.weak(&run.event);
            if current {
                ui.colored_label(ui.visuals().hyperlink_color, "running");
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::Label::new(egui::RichText::new(short_sha(&run.head_sha)).monospace())
                    .selectable(true),
            );
            ui.weak(&run.created_at);
            ui.label(status_text(run));
        });
        ui.horizontal(|ui| {
            ui.hyperlink_to("View on GitHub", &run.html_url);
            show_install_button(ui, run, state);
        });
    });
}

fn short_sha(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}

fn status_text(run: &WorkflowRun) -> String {
    match &run.conclusion {
        Some(conclusion) => conclusion.clone(),
        None => run.status.clone(),
    }
}

#[cfg(target_os = "android")]
fn show_install_button(ui: &mut egui::Ui, run: &WorkflowRun, state: &mut State) {
    let succeeded = run.conclusion.as_deref() == Some("success");
    let installing = state
        .install
        .as_ref()
        .is_some_and(|install| install.run_id == run.id && !install.finished());
    let label = if installing {
        "Installing…"
    } else {
        "Install"
    };
    let response = ui.add_enabled(succeeded && !installing, egui::Button::new(label));
    if !succeeded {
        response.on_disabled_hover_text("This run did not finish successfully.");
    }
    if response.clicked() {
        state.install = Some(install::Install::start(run.id, short_sha(&run.head_sha)));
    }

    if let Some(install) = &state.install {
        if install.run_id == run.id {
            if let Some(error) = install.error() {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn show_install_button(ui: &mut egui::Ui, _run: &WorkflowRun, _state: &mut State) {
    ui.add_enabled(false, egui::Button::new("Install"))
        .on_disabled_hover_text("Installing a downloaded build is only supported on Android.");
}
