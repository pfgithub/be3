pub mod app;

block_editor_plugin::plugin!(app::SettingsApp, "../manifest.json");

#[cfg(test)]
mod tests;
