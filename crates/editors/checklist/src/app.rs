use std::rc::Rc;
use std::sync::Arc;

use block_client::blocks::checklist::{Checklist, ChecklistOperation};
use block_editor_plugin::beui::{Context, Rect};
use block_editor_plugin::EditorHost;

mod checklist_changes;
mod ui;

use checklist_changes::ChecklistChanges;
use ui::{ChecklistModel, ChecklistSnapshot, ChecklistUi};

#[derive(Default)]
pub struct ChecklistApp {
    ui: Option<ChecklistUi>,
    checklist: Option<Rc<BlockChecklist>>,
    changes: Option<ChecklistChanges>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl ChecklistApp {
    pub fn ui(&self) -> Option<&ChecklistUi> {
        self.ui.as_ref()
    }
}

struct BlockChecklist {
    block: block_client::BlockHandle<Checklist>,
    host: EditorHost,
}

impl BlockChecklist {
    fn operate(&self, operation: ChecklistOperation) {
        if self.host.editable() {
            self.block.operate(operation);
        }
    }
}

impl ChecklistModel for BlockChecklist {
    fn snapshot(&self) -> ChecklistSnapshot {
        self.block
            .read()
            .map_or_else(ChecklistSnapshot::default, |checklist| {
                ChecklistSnapshot::from(&*checklist)
            })
    }

    fn add(&self, text: String) {
        self.operate(ChecklistOperation::Add { text });
    }

    fn set_done(&self, index: u32, done: bool) {
        self.operate(ChecklistOperation::SetDone { index, done });
    }

    fn remove(&self, index: u32) {
        self.operate(ChecklistOperation::Remove { index });
    }

    fn clear_done(&self) {
        self.operate(ChecklistOperation::ClearDone);
    }
}

impl block_editor_plugin::BeuiApp for ChecklistApp {
    fn connect(
        &mut self,
        host: EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: uuid::Uuid,
    ) {
        let block = client.get_block(block_id);
        self.changes = Some(ChecklistChanges::new(block.clone(), host.waker()));
        self.checklist = Some(Rc::new(BlockChecklist { block, host }));
        self.ui = None;
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<block_client::BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<uuid::Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Checklist::default()).id())
    }

    fn frame(&mut self, context: &Context, rect: Rect) {
        if let Some(snapshot) = self.changes.as_mut().and_then(ChecklistChanges::take) {
            if self.ui.is_none() {
                if let Some(checklist) = &self.checklist {
                    self.ui = Some(ChecklistUi::new(checklist.clone()));
                }
            }
            if let Some(ui) = &mut self.ui {
                ui.set_snapshot(snapshot);
            }
        }
        let Some(ui) = &mut self.ui else {
            return;
        };
        ui.show(context, rect);
    }
}
