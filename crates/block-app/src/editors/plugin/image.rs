use block::BlockParent;
use block_client::blocks::image::Image;
use block_plugin_api::PluginManifest;
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use super::PluginEditor;
use crate::{
    editors::EditorAccess,
    platform::{FileFilter, PickedFile},
};

pub(in crate::editors) fn manifest() -> Arc<PluginManifest> {
    static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
    super::cached_manifest(
        &MANIFEST,
        include_str!("../../../../editors/image_block/manifest.json"),
    )
}

pub(in crate::editors) fn image_filter() -> FileFilter {
    FileFilter::new("Images", "Image", Image::FILE_EXTENSIONS, Image::MIME_TYPES)
}

pub(in crate::editors) fn imported_image(file: PickedFile) -> Image {
    let PickedFile { name, data } = file;
    Image::new(name, data)
}

pub(in crate::editors) fn create_image_block(
    editors: &mut EditorAccess<'_>,
    image: Image,
    parent: Uuid,
) -> Uuid {
    let block = editors.client().create_block(image);
    let id = block.id();
    block.set_parent(BlockParent::Uuid(parent));
    editors.insert(Box::new(PluginEditor::new(manifest(), Box::new(block))));
    id
}
