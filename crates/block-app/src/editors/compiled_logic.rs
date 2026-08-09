use std::collections::HashMap;

use block::Block;
use block_client::{
    blocks::{compiled_logic::CompiledLogic, logic_grid::LogicGrid},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_MEMORY, MaterialIcon};
use logicgame::{execution::Instruction, grid::ConnectionDirection};
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 640.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 20.0;
/// Room for the headings, the summary rows and the section spacing.
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 220.0;

/// A compiled program is only ever produced by compiling a grid, so this shows
/// what was built rather than offering a way to change it.
pub(super) struct CompiledLogicEditor {
    block: BlockHandle<CompiledLogic>,
    /// Handles kept only so the grid and the called components can be named
    /// without opening an editor for each of them.
    source: Option<BlockHandle<LogicGrid>>,
    calls: HashMap<Uuid, BlockHandle<CompiledLogic>>,
}

impl EditorKind for CompiledLogicEditor {
    type Block = CompiledLogic;

    const DISPLAY_NAME: &'static str = "Compiled Logic";
    const ICON: MaterialIcon = ICON_MEMORY;

    fn open(_client: &BlockClient, block: BlockHandle<CompiledLogic>) -> Self {
        Self {
            block,
            source: None,
            calls: HashMap::new(),
        }
    }
}

impl BlockEditor for CompiledLogicEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let compiled = self.block.read()?;
        let rows =
            compiled.program().instructions.len() + compiled.ports().len() + compiled.calls().len();
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_CHROME_HEIGHT + DIRECT_EDITOR_ROW_HEIGHT * rows as f32,
        ))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(compiled) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let source_id = compiled.source();
        let calls = compiled.calls().to_vec();
        let client = editors.client();
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
        let mut action = None;

        ui.horizontal(|ui| {
            ui.strong("Compiled from");
            let label = self
                .source
                .as_ref()
                .map(|source| super::BlockLabel::for_handle(editors.registry(), source));
            let name = label.as_ref().map_or_else(
                || egui::RichText::new("Loading…"),
                super::BlockLabel::rich_text,
            );
            if ui.link(name).clicked() {
                action = Some(EditorAction::OpenBlock {
                    id: source_id,
                    block_type: LogicGrid::TYPE_ID,
                });
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
                .map(|handle| super::BlockLabel::for_handle(editors.registry(), handle));
            let name = label.as_ref().map_or_else(
                || egui::RichText::new("Loading…"),
                super::BlockLabel::rich_text,
            );
            if ui.link(name).clicked() {
                action = Some(EditorAction::OpenBlock {
                    id: *called,
                    block_type: CompiledLogic::TYPE_ID,
                });
            }
        }

        ui.add_space(8.0);
        ui.heading("Program");
        for (index, instruction) in compiled.program().instructions.iter().enumerate() {
            ui.monospace(format!("{index:>4}  {}", format_instruction(instruction)));
        }

        action
    }
}

/// One instruction on one line, short enough to read down a listing.
pub(super) fn format_instruction(instruction: &Instruction) -> String {
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

#[cfg(test)]
mod tests;
