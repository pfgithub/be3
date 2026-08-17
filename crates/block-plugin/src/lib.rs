pub use eframe::egui;

#[cfg(any(target_arch = "wasm32", target_os = "windows"))]
mod egui_session;
pub mod native;
#[cfg(not(target_arch = "wasm32"))]
mod runner;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_os = "windows")]
mod windows_surface;

pub trait App: Default + 'static {
    fn ui(&mut self, ui: &mut egui::Ui);
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
        use block_plugin::__private::{js_sys, wasm_bindgen, wasm_bindgen_futures};

        #[cfg(target_arch = "wasm32")]
        #[block_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub async fn start(
            canvas_id: String,
        ) -> Result<(), block_plugin::__private::wasm_bindgen::JsValue> {
            block_plugin::__private::start::<$app>(canvas_id).await
        }

        #[cfg(target_arch = "wasm32")]
        #[block_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn hello() -> Result<Vec<u8>, block_plugin::__private::wasm_bindgen::JsValue> {
            block_plugin::__private::hello($id, $name, env!("CARGO_PKG_VERSION"))
        }

        #[cfg(target_arch = "wasm32")]
        #[block_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn receive(
            frame: Vec<u8>,
        ) -> Result<
            block_plugin::__private::js_sys::Array,
            block_plugin::__private::wasm_bindgen::JsValue,
        > {
            block_plugin::__private::receive(frame)
        }

        #[cfg(target_arch = "wasm32")]
        #[block_plugin::__private::wasm_bindgen::prelude::wasm_bindgen]
        pub fn shutdown() {
            block_plugin::__private::shutdown();
        }

        #[cfg(not(target_arch = "wasm32"))]
        pub fn run() {
            block_plugin::__private::run::<$app>($id, $name, env!("CARGO_PKG_VERSION"));
        }
    };
}
