pub mod app;
mod kanban;
mod scatter;
mod spreadsheet;

block_editor_plugin::plugin!(app::DatabaseViewApp, "../manifest.json");

#[cfg(test)]
mod tests;
