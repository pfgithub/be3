pub mod app;
mod raytracer;

#[cfg(test)]
mod tests;

block_editor_plugin::plugin!(app::PixelRayTracerApp, "../manifest.json");
