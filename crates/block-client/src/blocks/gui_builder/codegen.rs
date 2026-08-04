use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use super::{GuiBuilder, GuiLayout, GuiWidget, GuiWidgetKind};

const DEFAULT_STRUCT_NAME: &str = "GuiWindow";
const INDENT: &str = "    ";

/// Rust identifiers that cannot be used as struct or field names.
const RESERVED: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while",
];

/// A widget that holds state needs somewhere to keep it, so it becomes a
/// field on the generated struct.
struct Field {
    name: String,
    declaration: String,
    default: String,
}

pub(super) fn generate(builder: &GuiBuilder) -> String {
    let struct_name = pascal_case(builder.title());
    let mut fields = Vec::new();
    let mut names = HashSet::new();
    let mut bindings = HashMap::new();
    collect_fields(builder.widgets(), &mut fields, &mut names, &mut bindings);

    let mut code = String::new();
    code.push_str("// Generated from a GUI builder block.\n");
    code.push_str("use eframe::egui;\n\n");

    code.push_str(&format!("pub struct {struct_name} {{\n"));
    for field in &fields {
        code.push_str(&format!(
            "{INDENT}pub {}: {},\n",
            field.name, field.declaration
        ));
    }
    code.push_str("}\n\n");

    code.push_str(&format!("impl Default for {struct_name} {{\n"));
    code.push_str(&format!("{INDENT}fn default() -> Self {{\n"));
    code.push_str(&format!("{INDENT}{INDENT}Self {{\n"));
    for field in &fields {
        code.push_str(&format!(
            "{INDENT}{INDENT}{INDENT}{}: {},\n",
            field.name, field.default
        ));
    }
    code.push_str(&format!("{INDENT}{INDENT}}}\n"));
    code.push_str(&format!("{INDENT}}}\n"));
    code.push_str("}\n\n");

    code.push_str(&format!("impl {struct_name} {{\n"));
    code.push_str(&format!(
        "{INDENT}pub fn show(&mut self, ui: &mut egui::Ui) {{\n"
    ));
    if builder.widgets().is_empty() {
        code.push_str(&format!("{INDENT}{INDENT}// No widgets yet.\n"));
    }
    emit(builder.widgets(), &bindings, 2, &mut code);
    code.push_str(&format!("{INDENT}}}\n"));
    code.push_str("}\n");
    code
}

fn collect_fields(
    widgets: &[GuiWidget],
    fields: &mut Vec<Field>,
    names: &mut HashSet<String>,
    bindings: &mut HashMap<Uuid, String>,
) {
    for widget in widgets {
        let field = match &widget.kind {
            GuiWidgetKind::TextField { label, value, .. } => Some(Field {
                name: unique_name(label, "text", names),
                declaration: "String".into(),
                default: if value.is_empty() {
                    "String::new()".into()
                } else {
                    format!("{value:?}.into()")
                },
            }),
            GuiWidgetKind::Checkbox { label, checked } => Some(Field {
                name: unique_name(label, "checked", names),
                declaration: "bool".into(),
                default: checked.to_string(),
            }),
            GuiWidgetKind::Slider { label, value, .. } => Some(Field {
                name: unique_name(label, "value", names),
                declaration: "f32".into(),
                default: number(*value),
            }),
            _ => None,
        };
        if let Some(field) = field {
            bindings.insert(widget.id, field.name.clone());
            fields.push(field);
        }
        if widget.kind.is_container() {
            collect_fields(&widget.children, fields, names, bindings);
        }
    }
}

fn emit(widgets: &[GuiWidget], bindings: &HashMap<Uuid, String>, depth: usize, code: &mut String) {
    for widget in widgets {
        emit_widget(widget, bindings, depth, code);
    }
}

fn emit_widget(
    widget: &GuiWidget,
    bindings: &HashMap<Uuid, String>,
    depth: usize,
    code: &mut String,
) {
    let pad = INDENT.repeat(depth);
    // Stateful widgets always have a binding; the fallback keeps the output
    // compilable if one is ever missing.
    let binding = bindings
        .get(&widget.id)
        .cloned()
        .unwrap_or_else(|| "unused".into());
    match &widget.kind {
        GuiWidgetKind::Heading { text } => {
            code.push_str(&format!("{pad}ui.heading({text:?});\n"));
        }
        GuiWidgetKind::Label { text } => {
            code.push_str(&format!("{pad}ui.label({text:?});\n"));
        }
        GuiWidgetKind::Button { text } => {
            code.push_str(&format!("{pad}if ui.button({text:?}).clicked() {{\n"));
            code.push_str(&format!("{pad}{INDENT}// TODO: handle the click.\n"));
            code.push_str(&format!("{pad}}}\n"));
        }
        GuiWidgetKind::TextField {
            label, multiline, ..
        } => {
            let editor = if *multiline {
                format!("ui.text_edit_multiline(&mut self.{binding});")
            } else {
                format!("ui.text_edit_singleline(&mut self.{binding});")
            };
            if label.is_empty() {
                code.push_str(&format!("{pad}{editor}\n"));
            } else if *multiline {
                code.push_str(&format!("{pad}ui.label({label:?});\n"));
                code.push_str(&format!("{pad}{editor}\n"));
            } else {
                code.push_str(&format!("{pad}ui.horizontal(|ui| {{\n"));
                code.push_str(&format!("{pad}{INDENT}ui.label({label:?});\n"));
                code.push_str(&format!("{pad}{INDENT}{editor}\n"));
                code.push_str(&format!("{pad}}});\n"));
            }
        }
        GuiWidgetKind::Checkbox { label, .. } => {
            code.push_str(&format!(
                "{pad}ui.checkbox(&mut self.{binding}, {label:?});\n"
            ));
        }
        GuiWidgetKind::Slider {
            label, min, max, ..
        } => {
            let range = format!("{}..={}", number(*min), number(*max));
            if label.is_empty() {
                code.push_str(&format!(
                    "{pad}ui.add(egui::Slider::new(&mut self.{binding}, {range}));\n"
                ));
            } else {
                code.push_str(&format!(
                    "{pad}ui.add(egui::Slider::new(&mut self.{binding}, {range}).text({label:?}));\n"
                ));
            }
        }
        GuiWidgetKind::Separator => code.push_str(&format!("{pad}ui.separator();\n")),
        GuiWidgetKind::Space { height } => {
            code.push_str(&format!("{pad}ui.add_space({});\n", number(*height)));
        }
        GuiWidgetKind::Container { layout, framed } => {
            if *framed {
                code.push_str(&format!(
                    "{pad}egui::Frame::group(ui.style()).show(ui, |ui| {{\n"
                ));
                emit_layout(widget, *layout, bindings, depth + 1, code);
                code.push_str(&format!("{pad}}});\n"));
            } else {
                emit_layout(widget, *layout, bindings, depth, code);
            }
        }
    }
}

fn emit_layout(
    widget: &GuiWidget,
    layout: GuiLayout,
    bindings: &HashMap<Uuid, String>,
    depth: usize,
    code: &mut String,
) {
    let pad = INDENT.repeat(depth);
    let method = match layout {
        GuiLayout::Vertical => "vertical",
        GuiLayout::Horizontal => "horizontal",
    };
    code.push_str(&format!("{pad}ui.{method}(|ui| {{\n"));
    emit(&widget.children, bindings, depth + 1, code);
    code.push_str(&format!("{pad}}});\n"));
}

fn unique_name(label: &str, fallback: &str, names: &mut HashSet<String>) -> String {
    let base = snake_case(label, fallback);
    // The field would otherwise collide with the generated `show` method.
    let base = if base == "show" {
        "show_value".to_owned()
    } else {
        base
    };
    let mut name = base.clone();
    let mut suffix = 2;
    while !names.insert(name.clone()) {
        name = format!("{base}_{suffix}");
        suffix += 1;
    }
    name
}

fn snake_case(text: &str, fallback: &str) -> String {
    let mut name = String::new();
    let mut pending_separator = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_separator && !name.is_empty() {
                name.push('_');
            }
            pending_separator = false;
            name.push(character.to_ascii_lowercase());
        } else {
            pending_separator = true;
        }
    }
    if name.is_empty() || name.starts_with(|character: char| character.is_ascii_digit()) {
        name = format!("{fallback}_{name}");
        name = name.trim_end_matches('_').to_owned();
    }
    if RESERVED.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

fn pascal_case(text: &str) -> String {
    let mut name = String::new();
    let mut capitalize = true;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            if capitalize {
                name.push(character.to_ascii_uppercase());
            } else {
                name.push(character);
            }
            capitalize = false;
        } else {
            capitalize = true;
        }
    }
    if name.is_empty() || name.starts_with(|character: char| character.is_ascii_digit()) {
        return DEFAULT_STRUCT_NAME.to_owned();
    }
    if RESERVED.contains(&name.as_str()) {
        name.push('_');
    }
    name
}

fn number(value: f32) -> String {
    if !value.is_finite() {
        return "0.0".into();
    }
    if value.fract() == 0.0 {
        format!("{value:.1}")
    } else {
        format!("{value}")
    }
}

#[cfg(test)]
mod tests;
