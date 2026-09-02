pub mod app;
mod font;
mod hex;
mod timings;

block_editor_plugin::plugin!(app::TextApp, "../manifest.json");
