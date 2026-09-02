mod access;
pub mod app;
mod images;
mod viewport;

block_editor_plugin::plugin!(app::CanvasApp, "../manifest.json");
