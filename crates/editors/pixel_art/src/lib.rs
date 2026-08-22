pub mod app;
pub mod artifact;
pub mod canvas;
pub mod color;
pub mod drawing;
pub mod panels;

block_editor_plugin::plugin!(app::PixelArtApp, "../manifest.json");
