use block::{Block, BlockParent};
use block_client::{blocks::image::Image, BlockClient, BlockHandle};
use block_plugin_api::{
    ChildOperations, CreationMode, EditorCapabilities, EditorRegion, EntryPoints, InteractionMode,
    PluginIdentity, PluginManifest, ResizeMode, SurfaceMechanism,
};
use egui_material_icons::{icons::ICON_IMAGE, MaterialIcon};
use std::sync::{Arc, OnceLock};
use uuid::Uuid;

use super::{PluginEditor, PluginPackage};
use crate::{
    editors::EditorAccess,
    platform::{FileFilter, PickedFile},
};

pub(in crate::editors) struct ImagePlugin;

impl PluginPackage for ImagePlugin {
    type Block = Image;

    const ICON: MaterialIcon = ICON_IMAGE;

    fn new_block(_client: &BlockClient) -> Option<BlockHandle<Image>> {
        None
    }

    fn manifest() -> Arc<PluginManifest> {
        static MANIFEST: OnceLock<Arc<PluginManifest>> = OnceLock::new();
        super::cached_manifest(&MANIFEST, || PluginManifest {
            identity: PluginIdentity {
                id: "be3.image".into(),
                name: "Image".into(),
                version: "1".into(),
            },
            block_type: Image::TYPE_ID.into_bytes(),
            display_name: "Image".into(),
            icon: ICON_IMAGE.codepoint.into(),
            creation: CreationMode::Dialog,
            children: ChildOperations::default(),
            important: false,
            interaction: InteractionMode::Preview,
            capabilities: EditorCapabilities {
                rotation: false,
                preserve_aspect_ratio: true,
                pan_and_zoom: false,
            },
            resize: ResizeMode::None,
            regions: vec![
                EditorRegion::Main,
                EditorRegion::RightSidebar,
                EditorRegion::Preview,
            ],
            entry_points: EntryPoints {
                web: Some("/image_block.js".into()),
                windows: Some("image_block-host.exe".into()),
            },
            surfaces: vec![
                SurfaceMechanism::WebExternalImage,
                SurfaceMechanism::WindowsDxgi,
            ],
        })
    }
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
    editors.insert(Box::new(PluginEditor::<ImagePlugin>::new(block)));
    id
}
