use std::sync::Arc;

use block_client::blocks::counter::{Counter, CounterOperation};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui;

pub struct CounterApp {
    block: Option<block_client::BlockHandle<Counter>>,
    creation: Option<Arc<block_client::BlockClient>>,
    step: i64,
    applied: u64,
}

impl Default for CounterApp {
    fn default() -> Self {
        Self {
            block: None,
            creation: None,
            step: 1,
            applied: 0,
        }
    }
}

impl CounterApp {
    fn count(&self) -> Option<i64> {
        let block = self.block.as_ref()?;
        let counter = block.read()?;
        let count = counter.count();
        drop(counter);
        Some(count)
    }

    fn apply(&mut self, operation: CounterOperation) {
        let Some(block) = &self.block else {
            return;
        };
        for _ in 0..self.step.max(1) {
            block.operate(operation.clone());
        }
        self.applied += 1;
    }
}

impl block_editor_plugin::App for CounterApp {
    fn connect(
        &mut self,
        _host: block_editor_plugin::EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: uuid::Uuid,
    ) {
        self.block = Some(client.get_block(block_id));
    }

    fn connect_creation(
        &mut self,
        _host: block_editor_plugin::EditorHost,
        client: Arc<block_client::BlockClient>,
    ) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<uuid::Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Counter::default()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(count) = self.count() else {
            ui.spinner();
            return;
        };
        ui.centered_and_justified(|ui| {
            ui.horizontal(|ui| {
                if ui.button("Remove").test_id("counter.decrement").clicked() {
                    #[cfg(target_os = "windows")]
                    eprintln!("plugin input guest activated remove");
                    self.apply(CounterOperation::Decrement);
                }
                ui.label(count.to_string()).test_id("counter.count");
                if ui.button("Add").test_id("counter.increment").clicked() {
                    #[cfg(target_os = "windows")]
                    eprintln!("plugin input guest activated add");
                    self.apply(CounterOperation::Increment);
                }
            });
        });
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(count) = self.count() else {
            ui.spinner();
            return;
        };
        ui.horizontal(|ui| {
            if ui.button("-").clicked() {
                self.apply(CounterOperation::Decrement);
            }
            if ui.button("+").clicked() {
                self.apply(CounterOperation::Increment);
            }
            ui.separator();
            ui.label(format!("Count: {count}"));
            ui.separator();
            ui.label(format!("Step: {}", self.step));
        });
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Step");
        for step in [1, 5, 10] {
            if ui
                .selectable_label(self.step == step, step.to_string())
                .test_id(&format!("counter.step.{step}"))
                .clicked()
            {
                self.step = step;
            }
        }
        ui.separator();
        ui.heading("Actions");
        if ui.button("Add step").test_id("counter.add-step").clicked() {
            self.apply(CounterOperation::Increment);
        }
        if ui.button("Remove step").clicked() {
            self.apply(CounterOperation::Decrement);
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Details");
        match self.count() {
            Some(count) => {
                ui.label(format!("Value: {count}"));
                ui.label(format!(
                    "Parity: {}",
                    if count % 2 == 0 { "even" } else { "odd" }
                ));
                ui.label(format!("Doubled: {}", count.saturating_mul(2)));
            }
            None => {
                ui.spinner();
            }
        }
        ui.separator();
        ui.label(format!("Edits this session: {}", self.applied));
    }
}
