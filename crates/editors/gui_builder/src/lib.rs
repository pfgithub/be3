pub mod app;
mod artifact;
mod inspector;
mod surface;

block_editor_plugin::plugin!(app::GuiBuilderApp, "../manifest.json");

#[cfg(test)]
mod tests;
