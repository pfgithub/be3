pub mod app;
pub mod download;
pub mod render;

#[cfg(test)]
mod tests;

block_editor_plugin::plugin!(app::PaintReviewApp, "../manifest.json");
