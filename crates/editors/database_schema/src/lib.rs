pub mod app;

block_editor_plugin::plugin!(app::DatabaseSchemaApp, "../manifest.json");

#[cfg(test)]
mod tests;
