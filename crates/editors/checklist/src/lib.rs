pub mod app;

block_editor_plugin::beui_plugin!(app::ChecklistApp, "../manifest.json");

#[cfg(test)]
mod tests;
