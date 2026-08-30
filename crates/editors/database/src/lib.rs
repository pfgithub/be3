pub mod app;

block_editor_plugin::plugin!(app::DatabaseApp, "../manifest.json");

#[cfg(test)]
mod tests;
