pub mod app;
pub mod demo;

block_editor_plugin::beui_plugin!(app::CounterApp, "../manifest.json");

#[cfg(test)]
mod tests;
