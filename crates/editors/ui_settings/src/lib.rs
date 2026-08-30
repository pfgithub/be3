pub mod app;

block_editor_plugin::plugin!(app::UiSettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
