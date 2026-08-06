use block::Block;
use block_client::{
    blocks::{
        gui_builder::GuiBuilder,
        text::{TextDocument, TextLanguage},
    },
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use eframe::egui;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::super::{DynamicArtifactRegeneration, DynamicArtifactSupport};

/// The descriptor payload: which design the code came from, and how it is
/// generated.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct CodeArtifact {
    source: Uuid,
    settings: CodeSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct CodeSettings {
    /// Empty means the struct is named after the design title.
    struct_name: String,
    /// Whether regenerating also renames the artifact after the design.
    rename_with_source: bool,
}

impl Default for CodeSettings {
    fn default() -> Self {
        Self {
            struct_name: String::new(),
            rename_with_source: true,
        }
    }
}

impl CodeArtifact {
    fn decode(data: &[u8]) -> Result<Self, String> {
        serde_json::from_slice(data)
            .map_err(|error| format!("GUI builder code descriptor is unreadable: {error}"))
    }

    fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

pub(in crate::editors) const SUPPORT: DynamicArtifactSupport = DynamicArtifactSupport {
    source: |data| CodeArtifact::decode(data).map(|artifact| artifact.source),
    summary,
    settings_ui,
    regenerate,
};

pub(super) fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: GuiBuilder::TYPE_ID,
        data: CodeArtifact {
            source: source_id,
            settings: CodeSettings::default(),
        }
        .encode(),
    }
}

/// The code a freshly exported artifact starts with.
pub(super) fn generate_initial(builder: &GuiBuilder) -> TextDocument {
    generate(builder, &CodeSettings::default())
}

fn generate(builder: &GuiBuilder, settings: &CodeSettings) -> TextDocument {
    TextDocument::from_bytes(builder.generate_code(Some(&settings.struct_name)))
        .with_language(TextLanguage::Rust)
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Code")
}

fn summary(data: &[u8]) -> String {
    let Ok(artifact) = CodeArtifact::decode(data) else {
        return "Rust code".to_owned();
    };
    if artifact.settings.struct_name.trim().is_empty() {
        "Rust code named after the design".to_owned()
    } else {
        format!("Rust code for `{}`", artifact.settings.struct_name.trim())
    }
}

fn settings_ui(ui: &mut egui::Ui, data: &mut Vec<u8>) -> bool {
    let Ok(mut artifact) = CodeArtifact::decode(data) else {
        ui.label("These settings cannot be read.");
        return false;
    };
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label("Struct name");
        changed |= ui
            .add(
                egui::TextEdit::singleline(&mut artifact.settings.struct_name)
                    .hint_text("From the design title"),
            )
            .changed();
    });
    changed |= ui
        .checkbox(
            &mut artifact.settings.rename_with_source,
            "Rename with the design",
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
    if target_type != TextDocument::TYPE_ID {
        return Err(format!(
            "GUI builder code generation expected a Text target, found {target_type}"
        ));
    }
    let artifact = CodeArtifact::decode(data)?;
    Ok(Box::new(GuiBuilderRegeneration {
        source: client.get_block::<GuiBuilder>(artifact.source),
        target: client.get_block::<TextDocument>(target_id),
        settings: artifact.settings,
    }))
}

struct GuiBuilderRegeneration {
    source: BlockHandle<GuiBuilder>,
    target: BlockHandle<TextDocument>,
    settings: CodeSettings,
}

impl DynamicArtifactRegeneration for GuiBuilderRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;
        // Both blocks have to be resolved before the target is overwritten.
        self.target.read()?;
        let generated = generate(&source, &self.settings);
        drop(source);
        self.target.replace(generated);
        if self.settings.rename_with_source {
            let source_name = self
                .source
                .name()
                .unwrap_or_else(|| "GUI Builder".to_owned());
            self.target.set_name(artifact_name(&source_name));
        }
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests;
