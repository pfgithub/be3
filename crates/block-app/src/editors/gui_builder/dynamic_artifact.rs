use block::Block;
use block_client::{
    blocks::{
        gui_builder::GuiBuilder,
        text::{TextDocument, TextLanguage},
    },
    BlockClient, BlockHandle, DynamicArtifactDescriptor,
};
use uuid::Uuid;

use super::super::DynamicArtifactRegeneration;

pub(super) fn descriptor(source_id: Uuid) -> DynamicArtifactDescriptor {
    DynamicArtifactDescriptor {
        source_type: GuiBuilder::TYPE_ID,
        data: source_id.as_bytes().to_vec(),
    }
}

pub(super) fn generate(builder: &GuiBuilder) -> TextDocument {
    TextDocument::from_bytes(builder.generate_code()).with_language(TextLanguage::Rust)
}

pub(super) fn artifact_name(source_name: &str) -> String {
    format!("{source_name} Code")
}

pub(in crate::editors) fn regenerate(
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
    let source_id = Uuid::from_slice(data)
        .map_err(|_| "GUI builder code descriptor has an invalid source id".to_owned())?;
    Ok(Box::new(GuiBuilderRegeneration {
        source: client.get_block::<GuiBuilder>(source_id),
        target: client.get_block::<TextDocument>(target_id),
    }))
}

struct GuiBuilderRegeneration {
    source: BlockHandle<GuiBuilder>,
    target: BlockHandle<TextDocument>,
}

impl DynamicArtifactRegeneration for GuiBuilderRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>> {
        let source = self.source.read()?;
        // Both blocks have to be resolved before the target is overwritten.
        self.target.read()?;
        let generated = generate(&source);
        drop(source);
        self.target.replace(generated);
        self.target.set_name(artifact_name(&self.source.name()));
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests;
