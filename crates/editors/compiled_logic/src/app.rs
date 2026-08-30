use std::collections::HashMap;
use std::sync::Arc;

use block::Block;
use block_client::blocks::compiled_logic::CompiledLogic;
use block_client::blocks::logic_grid::LogicGrid;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::{egui, EditorHost};
use logicgame::execution::Instruction;
use logicgame::grid::ConnectionDirection;
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 640.0;
const ROW_HEIGHT: f32 = 20.0;
const CHROME_HEIGHT: f32 = 220.0;

#[derive(Default)]
pub struct CompiledLogicApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<CompiledLogic>>,
    source: Option<BlockHandle<LogicGrid>>,
    calls: HashMap<Uuid, BlockHandle<CompiledLogic>>,
}

impl block_editor_plugin::App for CompiledLogicApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let compiled = self.block.as_ref()?.read()?;
        let rows =
            compiled.program().instructions.len() + compiled.ports().len() + compiled.calls().len();
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            CHROME_HEIGHT + ROW_HEIGHT * rows as f32,
        ))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(client), Some(block)) =
            (self.host.clone(), self.client.clone(), self.block.clone())
        else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(compiled) = block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let source_id = compiled.source();
        let calls = compiled.calls().to_vec();
        if self
            .source
            .as_ref()
            .is_none_or(|source| source.id() != source_id)
        {
            self.source = Some(client.get_block::<LogicGrid>(source_id));
        }
        self.calls.retain(|called, _| calls.contains(called));
        for called in &calls {
            self.calls
                .entry(*called)
                .or_insert_with(|| client.get_block::<CompiledLogic>(*called));
        }
        let types = host.block_types();

        ui.horizontal(|ui| {
            ui.strong("Compiled from");
            let label = self
                .source
                .as_ref()
                .map(|source| BlockLabel::for_handle(types.as_ref(), source));
            let name = label
                .as_ref()
                .map_or_else(|| egui::RichText::new("Loading…"), BlockLabel::rich_text);
            if ui.link(name).clicked() {
                host.open_block(source_id, LogicGrid::TYPE_ID);
            }
        });
        ui.horizontal(|ui| {
            ui.strong("Size");
            let size = compiled.size();
            ui.label(format!("{} x {}", size.width, size.height));
            ui.separator();
            ui.strong("Memory");
            ui.label(compiled.program().memory_size.to_string());
            ui.separator();
            ui.strong("Storage");
            ui.label(compiled.program().storage_init.len().to_string());
        });

        ui.add_space(8.0);
        ui.heading("Ports");
        for port in compiled.ports() {
            let direction = match port.direction {
                ConnectionDirection::Input => "in",
                ConnectionDirection::Output => "out",
            };
            let label = if port.label.is_empty() {
                format!("{direction} {}", port.index)
            } else {
                format!("{direction} {} - {}", port.index, port.label)
            };
            ui.horizontal(|ui| {
                ui.monospace(label);
                ui.weak(format!("{:?}, {} bit", port.side, port.scale.get()));
            });
        }

        ui.add_space(8.0);
        ui.heading("Calls");
        if calls.is_empty() {
            ui.weak("This component calls nothing else.");
        }
        for called in &calls {
            let label = self
                .calls
                .get(called)
                .map(|handle| BlockLabel::for_handle(types.as_ref(), handle));
            let name = label
                .as_ref()
                .map_or_else(|| egui::RichText::new("Loading…"), BlockLabel::rich_text);
            if ui.link(name).clicked() {
                host.open_block(*called, CompiledLogic::TYPE_ID);
            }
        }

        ui.add_space(8.0);
        ui.heading("Program");
        for (index, instruction) in compiled.program().instructions.iter().enumerate() {
            ui.monospace(format!("{index:>4}  {}", format_instruction(instruction)));
        }
    }
}

pub fn format_instruction(instruction: &Instruction) -> String {
    match instruction {
        Instruction::Call {
            component,
            instance,
            subgraph,
            inputs,
            outputs,
            ..
        } => format!("CALL c{component} i{instance} g{subgraph} {inputs:?} -> {outputs:?}"),
        Instruction::Not { input, output } => format!("NOT m{input} -> m{output}"),
        Instruction::CopyBits {
            input,
            output,
            shift,
            mask,
        } => format!("BITS m{input} shift {shift} mask {mask:#x} -> m{output}"),
        Instruction::ReadStorage { storage, output } => format!("READ s{storage} -> m{output}"),
        Instruction::SaveStorage { storage, input } => format!("SAVE m{input} -> s{storage}"),
    }
}
