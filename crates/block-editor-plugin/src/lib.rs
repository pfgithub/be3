pub use eframe::egui;
pub use egui_material_icons;

#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod egui_session;
mod host;
pub mod native;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod panes;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod screens;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
mod web_surface;
#[cfg(target_os = "windows")]
mod windows_surface;

pub use block_plugin_api::EditorRegion;
pub use block_ui;
pub use host::{BlockDrag, EditorHost};

pub trait App: Default + 'static {
    fn connect(
        &mut self,
        _host: EditorHost,
        _client: block_client::BlockClient,
        _block_id: uuid::Uuid,
    ) {
    }
    fn ui(&mut self, ui: &mut egui::Ui);
    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
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
    pub fn hello(id: &str, name: &str, version: &str) -> Result<Vec<u8>, wasm_bindgen::JsValue> {
        crate::web::hello(id, name, version)
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
    pub fn run<A: crate::App>(id: &str, name: &str, version: &str) {
        crate::runner::run::<A>(id, name, version);
    }
}

#[macro_export]
macro_rules! plugin {
    ($app:ty, $id:expr, $name:expr) => {
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
            block_editor_plugin::__private::hello($id, $name, env!("CARGO_PKG_VERSION"))
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
            block_editor_plugin::__private::run::<$app>($id, $name, env!("CARGO_PKG_VERSION"));
        }
    };
}
