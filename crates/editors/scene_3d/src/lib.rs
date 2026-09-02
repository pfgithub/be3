pub mod app;
mod camera;
#[cfg(target_arch = "wasm32")]
mod renderer;
mod scene;

block_editor_plugin::plugin!(app::Scene3DApp, "../manifest.json");

#[cfg(test)]
mod tests;
