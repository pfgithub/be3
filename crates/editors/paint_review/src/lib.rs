pub mod app;
pub mod render;
pub mod scan;

#[cfg(test)]
mod tests;

block_editor_plugin::plugin!(app::PaintReviewApp, "../manifest.json");
