pub(super) mod dynamic_artifact;

use block::Block;
use block_client::{
    blocks::{compiled_logic::CompiledLogic, logic_grid::LogicGrid},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_BUILD, ICON_MEDIATION},
    MaterialIcon,
};
use logicgame::grid::ValidationError;

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport,
    DynamicArtifactSupport, EditorAccess, EditorAction, EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 640.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 20.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 120.0;

pub(super) struct LogicGridEditor {
    block: BlockHandle<LogicGrid>,
    compile_error: Option<String>,
}

impl EditorKind for LogicGridEditor {
    type Block = LogicGrid;

    const DISPLAY_NAME: &'static str = "Logic Grid";
    const ICON: MaterialIcon = ICON_MEDIATION;

    fn open(_client: &BlockClient, block: BlockHandle<LogicGrid>) -> Self {
        Self::new(block)
    }

    fn dynamic_artifact() -> Option<DynamicArtifactSupport> {
        Some(dynamic_artifact::SUPPORT)
    }
}

impl CreatableEditor for LogicGridEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(LogicGrid::new()))
    }
}

impl LogicGridEditor {
    fn new(block: BlockHandle<LogicGrid>) -> Self {
        Self {
            block,
            compile_error: None,
        }
    }

    /// Compiles the grid into a component another grid can call. The compiled
    /// block is an artifact of this one, so it is rebuilt from here rather than
    /// edited on its own.
    fn compile(&mut self, client: &BlockClient) -> Option<EditorAction> {
        let compiled = self
            .block
            .read()
            .map(|grid| dynamic_artifact::generate_initial(self.block.id(), &grid))?;
        match compiled {
            Ok(compiled) => {
                let child = client.create_dynamic_artifact(
                    compiled,
                    dynamic_artifact::descriptor(self.block.id()),
                );
                child.set_name(dynamic_artifact::artifact_name(&self.block.name()));
                self.compile_error = None;
                Some(EditorAction::OpenBlock {
                    id: child.id(),
                    block_type: CompiledLogic::TYPE_ID,
                })
            }
            Err(error) => {
                self.compile_error = Some(error);
                None
            }
        }
    }
}

impl BlockEditor for LogicGridEditor {
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
        let grid = self.block.read()?;
        let rows = grid.grid().components().count() + grid.grid().validate().len();
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_CHROME_HEIGHT + DIRECT_EDITOR_ROW_HEIGHT * rows as f32,
        ))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Compile", ICON_BUILD.codepoint))
                .on_hover_text("Build a component other grids can call")
                .clicked()
            {
                action = self.compile(editors.client());
            }
            if let Some(error) = &self.compile_error {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        action
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(block) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let grid = block.grid();

        ui.horizontal(|ui| {
            ui.strong("Components");
            ui.label(grid.components().count().to_string());
            ui.separator();
            ui.strong("Wires");
            ui.label(grid.wires().len().to_string());
        });

        let errors = grid.validate();
        ui.add_space(8.0);
        ui.heading("Problems");
        if errors.is_empty() {
            ui.weak("Nothing is overlapping or off its grid.");
        }
        for error in &errors {
            ui.colored_label(ui.visuals().error_fg_color, validation_message(error));
        }

        None
    }
}

fn validation_message(error: &ValidationError) -> String {
    match error {
        ValidationError::ComponentOverflow { component } => {
            format!("Component {} runs off the grid", component.0)
        }
        ValidationError::ComponentOverlap { first, second } => {
            format!("Components {} and {} overlap", first.0, second.0)
        }
        ValidationError::ComponentNotSnapped { component, snap } => {
            format!(
                "Component {} is not on its {}-cell grid",
                component.0,
                snap.get()
            )
        }
        ValidationError::WireComponentIntersection { component, .. } => {
            format!("A wire runs through component {}", component.0)
        }
        ValidationError::WireNotSnapped { .. } => "A wire is not on its grid".to_owned(),
        ValidationError::WireOverflow { .. } => "A wire runs off the grid".to_owned(),
        ValidationError::InfiniteLeadBlocked { component, blocker } => format!(
            "Component {} blocks the lead out of component {}",
            blocker.0, component.0
        ),
    }
}
