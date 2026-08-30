pub mod app;

block_editor_plugin::plugin!(app::CompiledLogicApp, "../manifest.json");

#[cfg(test)]
mod tests;
