pub use eframe::egui;
pub use egui_material_icons;

use std::sync::Arc;

#[cfg(target_arch = "wasm32")]
mod egui_session;
mod host;
#[cfg(target_arch = "wasm32")]
mod panes;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod punch;
#[cfg(target_arch = "wasm32")]
mod runtime;
#[cfg(target_arch = "wasm32")]
mod screens;
pub mod session;
#[cfg(target_arch = "wasm32")]
mod wasm;

pub use block_plugin_api::{
    AudioStatus, BlockFilter, BlockPick, ChildId, ChildLayer, ChildMode, ClipboardImage,
    EditorBand, EditorRegion, FetchResult, ViewChange,
};
pub use block_ui;
pub use host::{
    Artifact, ArtifactDescription, BlockDrag, BlockPicker, ChildHandle, EditorHost, FileDrop,
    FileFilter, FilePicker, ImagePaster, PastedImage, PerformanceMeasurementGuard,
    PerformanceReporter, PickedFile, Waker,
};

pub trait App: Default + 'static {
    fn connect(
        &mut self,
        _host: EditorHost,
        _client: Arc<block_client::BlockClient>,
        _block_id: uuid::Uuid,
    ) {
    }
    fn connect_creation(&mut self, _host: EditorHost, _client: Arc<block_client::BlockClient>) {}
    fn creation_ui(&mut self, _ui: &mut egui::Ui) {}
    fn create_block(&mut self) -> Result<uuid::Uuid, String> {
        Err("this editor does not create blocks".into())
    }
    fn connect_artifact(
        &mut self,
        _host: EditorHost,
        _client: Arc<block_client::BlockClient>,
        _artifact: Artifact,
    ) {
    }
    fn describe_artifact(&mut self, _data: &[u8]) -> Result<ArtifactDescription, String> {
        Err("this editor does not generate artifacts".into())
    }
    fn artifact_settings_ui(&mut self, _ui: &mut egui::Ui, _data: &mut Vec<u8>) {}
    fn regenerate_artifact(&mut self, _data: &[u8]) {}
    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        None
    }
    fn ui(&mut self, ui: &mut egui::Ui);
    fn preview_ui(&mut self, _ui: &mut egui::Ui) {}
    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        None
    }
    fn set_intrinsic_size(&mut self, _size: egui::Vec2) {}
    fn aspect_ratio(&mut self) -> Option<f32> {
        None
    }
    fn toolbar_ui(&mut self, _ui: &mut egui::Ui) {}
    fn left_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}
    fn right_sidebar_ui(&mut self, _ui: &mut egui::Ui) {}
}

#[doc(hidden)]
pub mod __private {
    #[cfg(target_arch = "wasm32")]
    pub fn initialize_tls(size: usize, align: usize) {
        crate::wasm::initialize_storage(size, align);
    }

    #[cfg(target_arch = "wasm32")]
    pub fn start_wasm<A: crate::App>(manifest: &str) {
        let document = document(manifest);
        let identity = document.identity();
        if let Err(error) = crate::wasm::start::<A>(
            &identity.id,
            &identity.name,
            &identity.version,
            document.chrome,
        ) {
            panic!("{} could not start: {error}", identity.name);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn step_wasm() {
        if let Err(error) = crate::wasm::step() {
            panic!("the plugin could not run a frame: {error}");
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn shutdown_wasm() {
        crate::wasm::shutdown();
    }

    pub fn document(manifest: &str) -> block_plugin_api::ManifestDocument {
        block_plugin_api::ManifestDocument::parse(manifest)
            .unwrap_or_else(|error| panic!("this plugin's manifest is invalid: {error}"))
    }

    pub fn identity(manifest: &str) -> block_plugin_api::PluginIdentity {
        document(manifest).identity()
    }
}

#[cfg(target_arch = "wasm32")]
#[macro_export]
macro_rules! platform_entry {
    ($app:ty, $manifest:ident) => {
        #[no_mangle]
        pub extern "C" fn plugin_initialize_tls(size: u32, align: u32) {
            $crate::__private::initialize_tls(size as usize, align as usize);
        }

        #[no_mangle]
        pub extern "C" fn plugin_start() {
            $crate::__private::start_wasm::<$app>($manifest);
        }

        #[no_mangle]
        pub extern "C" fn plugin_step() {
            $crate::__private::step_wasm();
        }

        #[no_mangle]
        pub extern "C" fn plugin_shutdown() {
            $crate::__private::shutdown_wasm();
        }
    };
}

#[cfg(not(target_arch = "wasm32"))]
#[macro_export]
macro_rules! platform_entry {
    ($app:ty, $manifest:ident) => {};
}

#[macro_export]
macro_rules! plugin {
    ($app:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        $crate::platform_entry!($app, PLUGIN_MANIFEST);
    };
}
