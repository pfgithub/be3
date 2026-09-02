pub mod app;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod frame;
mod paint;
#[cfg(target_arch = "wasm32")]
mod renderer;

block_editor_plugin::plugin!(app::LogicGridApp, "../manifest.json");
