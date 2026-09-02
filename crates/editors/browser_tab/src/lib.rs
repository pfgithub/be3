pub mod app;

block_editor_plugin::plugin!(app::BrowserTabApp, "../manifest.json");

#[cfg(test)]
mod tests;
