pub mod app;

block_editor_plugin::beui_plugin!(app::CounterApp, "../manifest.json");

#[cfg(test)]
mod tests;
