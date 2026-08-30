pub mod app;

block_editor_plugin::plugin!(app::VersionControlDataApp, "../manifest.json");

#[cfg(test)]
mod tests;
