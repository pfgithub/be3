pub mod app;
mod binary_addition;

block_editor_plugin::plugin!(app::LogicGameApp, "../manifest.json");

#[cfg(test)]
mod tests;
