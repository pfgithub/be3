pub mod app;
mod effects;
mod player;
mod timeline;

block_editor_plugin::plugin!(app::VideoApp, "../manifest.json");

#[cfg(test)]
mod tests;
