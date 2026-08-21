use block_client::blocks::checklist::{Checklist, ChecklistOperation};
use block_editor_plugin::egui;

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Filter {
    #[default]
    All,
    Open,
    Done,
}

impl Filter {
    const ALL: [(Self, &'static str); 3] = [
        (Self::All, "All"),
        (Self::Open, "Open"),
        (Self::Done, "Done"),
    ];

    fn keeps(self, done: bool) -> bool {
        match self {
            Self::All => true,
            Self::Open => !done,
            Self::Done => done,
        }
    }
}

#[derive(Default)]
pub struct ChecklistApp {
    block: Option<block_client::BlockHandle<Checklist>>,
    draft: String,
    filter: Filter,
}

impl ChecklistApp {
    fn items(&self) -> Option<Vec<(String, bool)>> {
        let checklist = self.block.as_ref()?.read()?;
        Some(
            checklist
                .items()
                .iter()
                .map(|item| (item.text.clone(), item.done))
                .collect(),
        )
    }

    fn apply(&self, operation: ChecklistOperation) {
        if let Some(block) = &self.block {
            block.operate(operation);
        }
    }

    fn add_draft(&mut self) {
        let text = self.draft.trim().to_owned();
        if text.is_empty() {
            return;
        }
        self.apply(ChecklistOperation::Add { text });
        self.draft.clear();
    }

    fn draft_ui(&mut self, ui: &mut egui::Ui) {
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.draft)
                .hint_text("New item")
                .desired_width(160.0),
        );
        let submitted =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
        if ui.button("Add").clicked() || submitted {
            self.add_draft();
        }
    }
}

impl block_editor_plugin::App for ChecklistApp {
    fn connect(
        &mut self,
        _host: block_editor_plugin::EditorHost,
        client: block_client::BlockClient,
        block_id: uuid::Uuid,
    ) {
        self.block = Some(client.get_block(block_id));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(items) = self.items() else {
            ui.spinner();
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut shown = 0;
            for (index, (text, done)) in items.iter().enumerate() {
                if !self.filter.keeps(*done) {
                    continue;
                }
                shown += 1;
                ui.horizontal(|ui| {
                    let mut checked = *done;
                    if ui.checkbox(&mut checked, text).changed() {
                        self.apply(ChecklistOperation::SetDone {
                            index: index as u32,
                            done: checked,
                        });
                    }
                    if ui.button("Remove").clicked() {
                        self.apply(ChecklistOperation::Remove {
                            index: index as u32,
                        });
                    }
                });
            }
            if shown == 0 {
                ui.label("Nothing here yet.");
            }
        });
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            self.draft_ui(ui);
            ui.separator();
            if ui.button("Clear done").clicked() {
                self.apply(ChecklistOperation::ClearDone);
            }
        });
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Show");
        for (filter, label) in Filter::ALL {
            if ui.selectable_label(self.filter == filter, label).clicked() {
                self.filter = filter;
            }
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Progress");
        let Some(items) = self.items() else {
            ui.spinner();
            return;
        };
        let done = items.iter().filter(|(_, done)| *done).count();
        ui.label(format!("Done: {done}"));
        ui.label(format!("Open: {}", items.len() - done));
        let fraction = if items.is_empty() {
            0.0
        } else {
            done as f32 / items.len() as f32
        };
        ui.add(egui::ProgressBar::new(fraction).show_percentage());
    }
}
