pub mod app;
mod geo;
mod mvt;
mod points;
mod raster;
mod sidebar;
mod tiles;

#[cfg(test)]
mod tests;

block_editor_plugin::plugin!(app::MapApp, "../manifest.json");
