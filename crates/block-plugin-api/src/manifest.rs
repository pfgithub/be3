use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ChildOperations, CreationMode, EditorCapabilities, EditorRegion, InteractionMode,
    ManifestError, PluginIdentity, PluginManifest, ResizeMode,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ManifestDocument {
    pub id: String,
    pub name: String,
    pub version: String,
    pub block_type: String,
    pub display_name: String,
    pub icon: String,
    pub creation: CreationMode,
    #[serde(default)]
    pub children: ChildOperations,
    #[serde(default)]
    pub important: bool,
    #[serde(default)]
    pub interaction: InteractionMode,
    #[serde(default)]
    pub capabilities: EditorCapabilities,
    #[serde(default)]
    pub resize: ResizeMode,
    pub regions: Vec<EditorRegion>,
    pub entry_point: String,
    #[serde(default)]
    pub network: Vec<String>,
}

impl ManifestDocument {
    pub fn parse(source: &str) -> Result<Self, ManifestError> {
        serde_json::from_str(source).map_err(|error| ManifestError::Malformed(error.to_string()))
    }

    pub fn identity(&self) -> PluginIdentity {
        PluginIdentity {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    pub fn into_manifest(self) -> Result<PluginManifest, ManifestError> {
        let block_type =
            Uuid::parse_str(&self.block_type).map_err(|_| ManifestError::InvalidBlockType)?;
        let manifest = PluginManifest {
            identity: PluginIdentity {
                id: self.id,
                name: self.name,
                version: self.version,
            },
            block_type: block_type.into_bytes(),
            display_name: self.display_name,
            icon: self.icon,
            creation: self.creation,
            children: self.children,
            important: self.important,
            interaction: self.interaction,
            capabilities: self.capabilities,
            resize: self.resize,
            regions: self.regions,
            entry_point: self.entry_point,
            network: self.network,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

pub fn manifest_from_json(source: &str) -> Result<PluginManifest, ManifestError> {
    ManifestDocument::parse(source)?.into_manifest()
}
