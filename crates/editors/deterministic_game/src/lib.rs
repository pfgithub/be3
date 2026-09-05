pub mod app;

block_editor_plugin::plugin!(app::DeterministicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
