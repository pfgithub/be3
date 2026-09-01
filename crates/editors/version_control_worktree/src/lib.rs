pub mod app;

block_editor_plugin::plugin!(app::VersionControlWorktreeApp, "../manifest.json");

#[cfg(test)]
mod tests;
