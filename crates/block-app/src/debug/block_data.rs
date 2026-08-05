use eframe::egui;
use serde_json::Value;
use uuid::Uuid;

use crate::BlockApp;

impl BlockApp {
    /// Shows a tab's block as its raw serialized data instead of its editor.
    pub(crate) fn show_debug_data(&mut self, ui: &mut egui::Ui, active: Uuid) {
        let data = self.client.block_debug_data(active);
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| match &data {
                Some(data) => show_debug_tree(ui, data),
                None => {
                    ui.weak("This block has not finished loading yet.");
                }
            });
    }
}

/// Renders a block's serialized data as a tree that can be expanded and
/// collapsed field by field, falling back to plain text if it turns out not
/// to be valid JSON.
fn show_debug_tree(ui: &mut egui::Ui, data: &str) {
    let root: Value = match serde_json::from_str(data) {
        Ok(root) => root,
        Err(_) => {
            ui.add(
                egui::Label::new(egui::RichText::new(data).monospace())
                    .wrap()
                    .selectable(true),
            );
            return;
        }
    };
    match &root {
        // The top-level object's own fields are shown directly, without an
        // enclosing node for the object itself.
        Value::Object(fields) if !fields.is_empty() => {
            for (key, value) in fields {
                show_node(ui, key, key, value, 0);
            }
        }
        _ => show_node(ui, "root", "root", &root, 0),
    }
}

/// Draws one field: a collapsible node for objects and arrays, or a single
/// row for a scalar. `path` uniquely identifies the node within the tree, so
/// egui can remember whether it is expanded across frames.
fn show_node(ui: &mut egui::Ui, path: &str, key: &str, value: &Value, depth: usize) {
    match value {
        Value::Object(fields) if !fields.is_empty() => {
            egui::CollapsingHeader::new(node_label(key, format!("{{{}}}", fields.len())))
                .id_salt(path)
                .default_open(depth == 0)
                .show(ui, |ui| {
                    for (child_key, child_value) in fields {
                        let child_path = format!("{path}.{child_key}");
                        show_node(ui, &child_path, child_key, child_value, depth + 1);
                    }
                });
        }
        Value::Array(items) if !items.is_empty() => {
            egui::CollapsingHeader::new(node_label(key, format!("[{}]", items.len())))
                .id_salt(path)
                .default_open(depth == 0)
                .show(ui, |ui| {
                    for (index, item) in items.iter().enumerate() {
                        let child_key = index.to_string();
                        let child_path = format!("{path}.{index}");
                        show_node(ui, &child_path, &child_key, item, depth + 1);
                    }
                });
        }
        Value::Object(_) => leaf_row(ui, key, "{}"),
        Value::Array(_) => leaf_row(ui, key, "[]"),
        scalar => leaf_row(ui, key, &scalar_text(scalar)),
    }
}

fn node_label(key: &str, summary: String) -> String {
    format!("{key} {summary}")
}

fn scalar_text(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("{value:?}"),
        Value::Object(_) | Value::Array(_) => unreachable!("handled by show_node"),
    }
}

fn leaf_row(ui: &mut egui::Ui, key: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.add(egui::Label::new(egui::RichText::new(key).monospace().strong()).selectable(true));
        ui.add(egui::Label::new(egui::RichText::new(text).monospace()).selectable(true));
    });
}
