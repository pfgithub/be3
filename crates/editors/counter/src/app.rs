use std::rc::Rc;
use std::sync::Arc;

use block_client::blocks::counter::{Counter as CounterBlock, CounterOperation};
use block_editor_plugin::beui::{Context, Rect};
use block_editor_plugin::EditorHost;

mod count_changes;
mod ui;

use count_changes::CountChanges;
use ui::{Counter, CounterUi};

#[derive(Default)]
pub struct CounterApp {
    ui: Option<CounterUi>,
    counter: Option<Rc<BlockCounter>>,
    changes: Option<CountChanges>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl CounterApp {
    pub fn ui(&self) -> Option<&CounterUi> {
        self.ui.as_ref()
    }
}

struct BlockCounter {
    block: block_client::BlockHandle<CounterBlock>,
    host: EditorHost,
}

impl BlockCounter {
    fn operate(&self, operation: CounterOperation) {
        if self.host.editable() {
            self.block.operate(operation);
        }
    }
}

impl Counter for BlockCounter {
    fn value(&self) -> i64 {
        self.block.read().map_or(0, |counter| counter.count())
    }

    fn increment(&self) {
        self.operate(CounterOperation::Increment);
    }

    fn decrement(&self) {
        self.operate(CounterOperation::Decrement);
    }

    fn reset(&self) {
        self.operate(CounterOperation::Reset);
    }
}

impl block_editor_plugin::BeuiApp for CounterApp {
    fn connect(
        &mut self,
        host: EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: uuid::Uuid,
    ) {
        let block = client.get_block(block_id);
        self.changes = Some(CountChanges::new(block.clone(), host.waker()));
        self.counter = Some(Rc::new(BlockCounter { block, host }));
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
        Ok(client.create_block(CounterBlock::default()).id())
    }

    fn frame(&mut self, context: &Context, rect: Rect) {
        if let Some(value) = self.changes.as_mut().and_then(CountChanges::take) {
            if self.ui.is_none() {
                if let Some(counter) = &self.counter {
                    self.ui = Some(CounterUi::new(counter.clone()));
                }
            }
            if let Some(ui) = &mut self.ui {
                ui.set_value(value);
            }
        }
        let Some(ui) = &mut self.ui else {
            return;
        };
        ui.show(context, rect);
    }
}
