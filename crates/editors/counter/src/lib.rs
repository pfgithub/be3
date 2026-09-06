pub mod app;

block_editor_plugin::plugin!(app::CounterApp, "../manifest.json");

#[cfg(test)]
mod tests;
