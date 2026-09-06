use std::rc::Rc;
use std::sync::Arc;

use beui::demo::{Counter, Demo};
use beui_egui::Canvas;
use block_client::blocks::counter::{Counter as CounterBlock, CounterOperation};
use block_editor_plugin::egui;

pub struct CounterApp {
    canvas: Canvas,
    demo: Option<Demo>,
    counter: Option<Rc<BlockCounter>>,
    creation: Option<Arc<block_client::BlockClient>>,
}

impl Default for CounterApp {
    fn default() -> Self {
        Self {
            canvas: Canvas::new(),
            demo: None,
            counter: None,
            creation: None,
        }
    }
}

impl CounterApp {
    pub fn demo(&self) -> Option<&Demo> {
        self.demo.as_ref()
    }
}

fn open(counter: Option<&Rc<BlockCounter>>) -> Option<Demo> {
    let counter = counter?;
    counter.block.read()?;
    let counter: Rc<dyn Counter> = counter.clone();
    Some(Demo::new(counter))
}

struct BlockCounter {
    block: block_client::BlockHandle<CounterBlock>,
    host: block_editor_plugin::EditorHost,
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

impl block_editor_plugin::App for CounterApp {
    fn connect(
        &mut self,
        host: block_editor_plugin::EditorHost,
        client: Arc<block_client::BlockClient>,
        block_id: uuid::Uuid,
    ) {
        self.counter = Some(Rc::new(BlockCounter {
            block: client.get_block(block_id),
            host,
        }));
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
        Ok(client.create_block(CounterBlock::default()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        if self.demo.is_none() {
            self.demo = open(self.counter.as_ref());
        }
        let Some(demo) = &mut self.demo else {
            return;
        };
        self.canvas
            .show(ui, |context, rect| demo.show(context, rect));
    }
}
