pub mod app;
mod pane;
mod render;

block_editor_plugin::plugin!(app::PdfApp, "../manifest.json");
