use block::Block;
use block_client::blocks::gui_builder::GuiBuilder;
use block_client::blocks::text::{TextDocument, TextLanguage};
use block_client::{BlockClient, BlockHandle, DynamicArtifactDescriptor};
use block_editor_plugin::{egui, ArtifactDescription};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct CodeArtifact {
    source: Uuid,
    settings: CodeSettings,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct CodeSettings {
    struct_name: String,

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

pub fn describe(data: &[u8]) -> Result<ArtifactDescription, String> {
    let artifact = CodeArtifact::decode(data)?;
    Ok(ArtifactDescription {
        source: artifact.source,
        summary: summary(&artifact.settings),
    })
}

pub fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: GuiBuilder::TYPE_ID,
        data: CodeArtifact {
            source: source_id,
            settings: CodeSettings::default(),
        }
        .encode(),
    }
}

pub fn generate_initial(builder: &GuiBuilder) -> TextDocument {
    generate(builder, &CodeSettings::default())
}

fn generate(builder: &GuiBuilder, settings: &CodeSettings) -> TextDocument {
    TextDocument::from_bytes(builder.generate_code(Some(&settings.struct_name)))
        .with_language(TextLanguage::Rust)
}

pub fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Code")
}

fn summary(settings: &CodeSettings) -> String {
    if settings.struct_name.trim().is_empty() {
        "Rust code named after the design".to_owned()
    } else {
        format!("Rust code for `{}`", settings.struct_name.trim())
    }
}

pub fn settings_ui(ui: &mut egui::Ui, data: &mut Vec<u8>) {
    let Ok(mut artifact) = CodeArtifact::decode(data) else {
        ui.label("These settings cannot be read.");
        return;
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
    ui.add_space(12.0);
    ui.weak(summary(&artifact.settings));
    if changed {
        *data = artifact.encode();
    }
}

pub struct Regeneration {
    source: BlockHandle<GuiBuilder>,
    target: BlockHandle<TextDocument>,
    settings: CodeSettings,
}

impl Regeneration {
    pub fn start(
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Self, String> {
        if target_type != TextDocument::TYPE_ID {
            return Err(format!(
                "GUI builder code generation expected a Text target, found {target_type}"
            ));
        }
        let artifact = CodeArtifact::decode(data)?;
        Ok(Self {
            source: client.get_block::<GuiBuilder>(artifact.source),
            target: client.get_block::<TextDocument>(target_id),
            settings: artifact.settings,
        })
    }

    pub fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;

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
