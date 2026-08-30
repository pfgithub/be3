use std::sync::Arc;

use block_client::blocks::compiled_logic::CompiledLogic;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::{App as _, EditorHost};
use block_ui_test::EditorTest;
use logicgame::execution::{Instruction, UnlinkedComponent};
use logicgame::grid::{ComponentPort, ComponentSide, ConnectionDirection, Scale, Size};
use uuid::Uuid;

use crate::app::{format_instruction, CompiledLogicApp};

mod instructions_have_compact_display_names;
mod the_program_lists_every_instruction;

fn compiled(source: Uuid) -> CompiledLogic {
    CompiledLogic::new(
        source,
        Size {
            width: 4,
            height: 3,
        },
        vec![
            ComponentPort {
                direction: ConnectionDirection::Input,
                index: 0,
                scale: Scale::ONE,
                side: ComponentSide::Left,
                start: 0,
                end: 1,
                label: "a".into(),
            },
            ComponentPort {
                direction: ConnectionDirection::Output,
                index: 0,
                scale: Scale::ONE,
                side: ComponentSide::Right,
                start: 0,
                end: 1,
                label: String::new(),
            },
        ],
        UnlinkedComponent {
            inputs: vec![0],
            outputs: vec![1],
            components: Vec::new(),
            instructions: vec![
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
                Instruction::SaveStorage {
                    storage: 0,
                    input: 1,
                },
            ],
            subgraphs: Vec::new(),
            memory_size: 2,
            storage_init: vec![0],
        },
    )
}

fn editor() -> (
    EditorTest<'static, CompiledLogicApp>,
    BlockHandle<CompiledLogic>,
) {
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    let block = client.create_block(compiled(Uuid::new_v4()));
    let host = EditorHost::default();
    host.set_editable(true);
    let mut app = CompiledLogicApp::default();
    app.connect(host, client, block.id());
    let mut editor = EditorTest::new(app);
    editor.run();
    (editor, block)
}
