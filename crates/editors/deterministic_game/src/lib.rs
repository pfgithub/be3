pub mod app;
pub(crate) mod catalog;

block_editor_plugin::plugin!(app::DeterministicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
