use block_client::blocks::counter::{Counter, CounterOperation};
use block_editor_plugin::egui;

#[derive(Default)]
pub struct CounterApp {
    block: Option<block_client::BlockHandle<Counter>>,
}

impl block_editor_plugin::App for CounterApp {
    fn connect(&mut self, client: block_client::BlockClient, block_id: uuid::Uuid) {
        self.block = Some(client.get_block(block_id));
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(block) = &self.block else {
            ui.spinner();
            return;
        };
        let Some(counter) = block.read() else {
            ui.spinner();
            return;
        };
        let count = counter.count();
        drop(counter);
        ui.centered_and_justified(|ui| {
            ui.horizontal(|ui| {
                if ui.button("Remove").clicked() {
                    block.operate(CounterOperation::Decrement);
                }
                ui.label(count.to_string());
                if ui.button("Add").clicked() {
                    block.operate(CounterOperation::Increment);
                }
            });
        });
    }
}
