use block::Block;
use block_client::blocks::gui_builder::{
    GuiBuilder, GuiBuilderOperation, GuiLocation, GuiWidget, GuiWidgetKind,
};

use super::{artifact_name, descriptor, generate, generate_initial, CodeArtifact, CodeSettings};

mod gui_builder_export_generates_the_designed_code;
mod struct_name_setting_renames_the_generated_struct;
