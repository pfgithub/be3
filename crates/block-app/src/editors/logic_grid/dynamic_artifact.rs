use block::Block;
use block_client::{
    blocks::{compiled_logic::CompiledLogic, logic_grid::LogicGrid},
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::{DynamicArtifactRegeneration, DynamicArtifactSupport};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ComponentArtifact {
    source: Uuid,
    settings: ComponentSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct ComponentSettings {
    rename_with_source: bool,
}

impl Default for ComponentSettings {
    fn default() -> Self {
        Self {
            rename_with_source: true,
        }
    }
}

impl ComponentArtifact {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("logic grid component descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub(in crate::editors) const SUPPORT: DynamicArtifactSupport = DynamicArtifactSupport {
    source: |data| ComponentArtifact::decode(data).map(|artifact| artifact.source),
    summary,
    settings_ui,
    regenerate,
};

pub(super) fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: LogicGrid::TYPE_ID,
        data: ComponentArtifact {
            source: source_id,
            settings: ComponentSettings::default(),
        }
        .encode(),
    }
}

pub(super) fn generate_initial(source_id: Uuid, grid: &LogicGrid) -> Result<CompiledLogic, String> {
    CompiledLogic::compile(source_id, grid.grid()).map_err(|error| error.to_string())
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Component")
}

fn summary(data: &[u8]) -> String {
    let Ok(artifact) = ComponentArtifact::decode(data) else {
        return "Compiled component".to_owned();
    };
    if artifact.settings.rename_with_source {
        "Compiled component, named after its grid".to_owned()
    } else {
        "Compiled component".to_owned()
    }
}

fn settings_ui(ui: &mut egui::Ui, data: &mut Vec<u8>) -> bool {
    let Ok(mut artifact) = ComponentArtifact::decode(data) else {
        ui.label("These settings cannot be read.");
        return false;
    };
    let changed = ui
        .checkbox(
            &mut artifact.settings.rename_with_source,
            "Rename with the grid",
        )
        .changed();
    if changed {
        *data = artifact.encode();
    }
    changed
}

fn regenerate(
    client: &BlockClient,
    target_id: Uuid,
    target_type: Uuid,
    data: &[u8],
) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
    if target_type != CompiledLogic::TYPE_ID {
        return Err(format!(
            "compiling a logic grid expected a Compiled Logic target, found {target_type}"
        ));
    }
    let artifact = ComponentArtifact::decode(data)?;
    Ok(Box::new(CompileRegeneration {
        source: client.get_block::<LogicGrid>(artifact.source),
        target: client.get_block::<CompiledLogic>(target_id),
        settings: artifact.settings,
    }))
}

struct CompileRegeneration {
    source: BlockHandle<LogicGrid>,
    target: BlockHandle<CompiledLogic>,
    settings: ComponentSettings,
}

impl DynamicArtifactRegeneration for CompileRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;

        self.target.read()?;
        let generated = generate_initial(self.source.id(), &source);
        drop(source);
        let compiled = match generated {
            Ok(compiled) => compiled,
            Err(error) => return Some(Err(error)),
        };
        self.target.replace(compiled);
        if self.settings.rename_with_source {
            let source_name = self
                .source
                .name()
                .unwrap_or_else(|| "Logic Grid".to_owned());
            self.target.set_name(artifact_name(&source_name));
        }
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests;
