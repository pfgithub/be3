pub mod app;
pub mod document;
mod font;
mod hex;
pub mod presence;
mod timings;

block_editor_plugin::plugin!(app::TextApp, "../manifest.json");
