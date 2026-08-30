pub mod app;

block_editor_plugin::plugin!(app::AudioApp, "../manifest.json");

#[cfg(test)]
mod tests;
