pub mod app;

block_editor_plugin::plugin!(app::GameModuleApp, "../manifest.json");

#[cfg(test)]
mod tests;
