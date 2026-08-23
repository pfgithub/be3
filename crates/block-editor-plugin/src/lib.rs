pub use eframe::egui;
pub use egui_material_icons;

use std::sync::Arc;

#[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
mod egui_session;
mod host;
#[cfg(target_os = "linux")]
mod linux_surface;
pub mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
mod panes;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(any(target_arch = "wasm32", target_os = "windows", target_os = "linux"))]
mod screens;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
mod web_surface;
#[cfg(target_os = "windows")]
mod windows_surface;

pub use block_plugin_api::EditorRegion;
pub use block_ui;
pub use host::{
    Artifact, ArtifactDescription, BlockDrag, EditorHost, FileFilter, FilePicker, PickedFile,
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
    pub use js_sys;
    #[cfg(target_arch = "wasm32")]
    pub use wasm_bindgen;
    #[cfg(target_arch = "wasm32")]
    pub use wasm_bindgen_futures;

    #[cfg(target_arch = "wasm32")]
    pub async fn start<A: crate::App>(canvas_id: String) -> Result<(), wasm_bindgen::JsValue> {
        crate::web::start::<A>(canvas_id).await
    }

    #[cfg(target_arch = "wasm32")]
    pub fn hello(manifest: &str) -> Result<Vec<u8>, wasm_bindgen::JsValue> {
        let identity = identity(manifest);
        crate::web::hello(&identity.id, &identity.name, &identity.version)
    }

    pub fn identity(manifest: &str) -> block_plugin_api::PluginIdentity {
        block_plugin_api::ManifestDocument::parse(manifest)
            .unwrap_or_else(|error| panic!("this plugin's manifest is invalid: {error}"))
            .identity()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn receive(frame: Vec<u8>) -> Result<js_sys::Array, wasm_bindgen::JsValue> {
        crate::web::receive(frame)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn poll() -> Result<js_sys::Array, wasm_bindgen::JsValue> {
        crate::web::poll()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn render() -> Result<js_sys::Array, wasm_bindgen::JsValue> {
        crate::web::render()
    }

    #[cfg(target_arch = "wasm32")]
    pub fn shutdown() {
        crate::web::shutdown();
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn run<A: crate::App>(manifest: &str) {
        let identity = identity(manifest);
        crate::runner::run::<A>(&identity.id, &identity.name, &identity.version);
    }
}

#[macro_export]
macro_rules! plugin {
    ($app:ty, $manifest:expr) => {
        const PLUGIN_MANIFEST: &str = include_str!($manifest);

        #[cfg(target_arch = "wasm32")]
        use block_editor_plugin::__private::{js_sys, wasm_bindgen, wasm_bindgen_futures};

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn start(
            canvas_id: String,
        ) -> Result<(), block_editor_plugin::__private::wasm_bindgen::JsValue> {
            block_editor_plugin::__private::start::<$app>(canvas_id).await
        }

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn hello() -> Result<Vec<u8>, block_editor_plugin::__private::wasm_bindgen::JsValue> {
            block_editor_plugin::__private::hello(PLUGIN_MANIFEST)
        }

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn receive(
            frame: Vec<u8>,
        ) -> Result<
            block_editor_plugin::__private::js_sys::Array,
            block_editor_plugin::__private::wasm_bindgen::JsValue,
        > {
            block_editor_plugin::__private::receive(frame)
        }

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn poll() -> Result<
            block_editor_plugin::__private::js_sys::Array,
            block_editor_plugin::__private::wasm_bindgen::JsValue,
        > {
            block_editor_plugin::__private::poll()
        }

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn render() -> Result<
            block_editor_plugin::__private::js_sys::Array,
            block_editor_plugin::__private::wasm_bindgen::JsValue,
        > {
            block_editor_plugin::__private::render()
        }

        #[cfg(target_arch = "wasm32")]
        #[block_editor_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn shutdown() {
            block_editor_plugin::__private::shutdown();
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn run() {
            block_editor_plugin::__private::run::<$app>(PLUGIN_MANIFEST);
        }
    };
}
