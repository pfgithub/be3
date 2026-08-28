pub mod demo;

block_editor_plugin::plugin!(demo::CounterApp, "../manifest.json");

#[cfg(test)]
mod tests;
