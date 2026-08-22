use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    ChildOperations, CreationMode, EditorCapabilities, EditorRegion, EntryPoints, InteractionMode,
    ManifestError, PluginIdentity, PluginManifest, ResizeMode, SurfaceMechanism,
};

#[cfg(test)]
mod tests;

/// A plugin's manifest as it is written by hand and shipped beside the
/// plugin: `block_type` as a uuid string, `icon` as the codepoint of the
/// host's icon font, and everything the host can default left out.
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
    #[serde(default)]
    pub entry_points: EntryPoints,
    #[serde(default)]
    pub surfaces: Vec<SurfaceMechanism>,
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
            entry_points: self.entry_points,
            surfaces: self.surfaces,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

/// Reads a manifest document and turns it into the manifest the host and the
/// plugin exchange. A manifest is untrusted input wherever it was found, so
/// every failure is answered rather than asserted.
pub fn manifest_from_json(source: &str) -> Result<PluginManifest, ManifestError> {
    ManifestDocument::parse(source)?.into_manifest()
}
