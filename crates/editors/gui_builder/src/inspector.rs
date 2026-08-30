use block_client::blocks::gui_builder::{
    GuiBuilder, GuiBuilderOperation, GuiLayout, GuiLocation, GuiWidget, GuiWidgetKind,
    MAX_CANVAS_SIZE, MAX_SPACE_HEIGHT, MIN_CANVAS_SIZE,
};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ARROW_DOWNWARD, ICON_ARROW_UPWARD, ICON_DELETE, ICON_FORMAT_INDENT_DECREASE,
    ICON_FORMAT_INDENT_INCREASE,
};
use uuid::Uuid;

use crate::app::{insertion_location, widget_icon};

const OUTLINE_INDENT: f32 = 12.0;

fn palette() -> Vec<(&'static str, GuiWidgetKind)> {
    vec![
        (
            "Heading",
            GuiWidgetKind::Heading {
                text: "Heading".into(),
            },
        ),
        (
            "Label",
            GuiWidgetKind::Label {
                text: "Label".into(),
            },
        ),
        (
            "Button",
            GuiWidgetKind::Button {
                text: "Button".into(),
            },
        ),
        (
            "Text field",
            GuiWidgetKind::TextField {
                label: "Text".into(),
                value: String::new(),
                multiline: false,
            },
        ),
        (
            "Checkbox",
            GuiWidgetKind::Checkbox {
                label: "Checkbox".into(),
                checked: false,
            },
        ),
        (
            "Slider",
            GuiWidgetKind::Slider {
                label: "Value".into(),
                value: 0.5,
                min: 0.0,
                max: 1.0,
            },
        ),
        ("Separator", GuiWidgetKind::Separator),
        ("Space", GuiWidgetKind::Space { height: 8.0 }),
        (
            "Column",
            GuiWidgetKind::Container {
                layout: GuiLayout::Vertical,
                framed: false,
            },
        ),
        (
            "Row",
            GuiWidgetKind::Container {
                layout: GuiLayout::Horizontal,
                framed: false,
            },
        ),
    ]
}

pub(crate) fn left_sidebar(
    ui: &mut egui::Ui,
    builder: &GuiBuilder,
    selected: &mut Option<Uuid>,
) -> Vec<GuiBuilderOperation> {
    let mut operations = Vec::new();
    ui.heading("Widgets");
    ui.add_space(4.0);
    for (label, kind) in palette() {
        let icon = widget_icon(&kind);
        if ui
            .add(
                egui::Button::new(format!("{} {label}", icon.codepoint))
                    .min_size(egui::vec2(ui.available_width(), 0.0)),
            )
            .test_id(&format!("gui-builder.palette.{label}"))
            .clicked()
        {
            let widget = GuiWidget::new(kind);
            let id = widget.id;
            operations.push(GuiBuilderOperation::Insert {
                location: insertion_location(builder, *selected),
                widget,
            });
            *selected = Some(id);
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.heading("Outline");
    ui.add_space(4.0);
    if builder.widgets().is_empty() {
        ui.weak("Add a widget to start the design.");
    }
    egui::ScrollArea::vertical()
        .id_salt("gui-builder-outline")
        .show(ui, |ui| {
            outline(ui, builder.widgets(), 0, selected);
        });

    if let Some(id) = *selected {
        ui.add_space(8.0);
        operations.extend(arrange(ui, builder, id, selected));
    }
    operations
}

fn outline(ui: &mut egui::Ui, widgets: &[GuiWidget], depth: usize, selected: &mut Option<Uuid>) {
    for widget in widgets {
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * OUTLINE_INDENT);
            let summary = widget.kind.summary();
            let label = if summary.is_empty() {
                format!(
                    "{} {}",
                    widget_icon(&widget.kind).codepoint,
                    widget.kind.display_name()
                )
            } else {
                format!("{} {summary}", widget_icon(&widget.kind).codepoint)
            };
            if ui
                .selectable_label(*selected == Some(widget.id), label)
                .clicked()
            {
                *selected = Some(widget.id);
            }
        });
        if widget.kind.is_container() {
            outline(ui, &widget.children, depth + 1, selected);
        }
    }
}

fn arrange(
    ui: &mut egui::Ui,
    builder: &GuiBuilder,
    id: Uuid,
    selected: &mut Option<Uuid>,
) -> Vec<GuiBuilderOperation> {
    let Some(location) = builder.location(id) else {
        return Vec::new();
    };
    let siblings = builder.children(location.parent).unwrap_or_default();
    let mut operations = Vec::new();
    ui.horizontal(|ui| {
        if ui
            .add_enabled(location.index > 0, egui::Button::new(ICON_ARROW_UPWARD))
            .on_hover_text("Move up")
            .clicked()
        {
            operations.push(GuiBuilderOperation::Move {
                id,
                location: GuiLocation::new(location.parent, location.index - 1),
            });
        }
        if ui
            .add_enabled(
                location.index + 1 < siblings.len(),
                egui::Button::new(ICON_ARROW_DOWNWARD),
            )
            .on_hover_text("Move down")
            .clicked()
        {
            operations.push(GuiBuilderOperation::Move {
                id,
                location: GuiLocation::new(location.parent, location.index + 1),
            });
        }

        let target = location
            .index
            .checked_sub(1)
            .and_then(|index| siblings.get(index))
            .filter(|sibling| sibling.kind.is_container());
        if ui
            .add_enabled(
                target.is_some(),
                egui::Button::new(ICON_FORMAT_INDENT_INCREASE),
            )
            .on_hover_text("Move into the container above")
            .clicked()
        {
            if let Some(target) = target {
                operations.push(GuiBuilderOperation::Move {
                    id,
                    location: GuiLocation::new(Some(target.id), target.children.len()),
                });
            }
        }

        let parent = location.parent.and_then(|parent| builder.location(parent));
        if ui
            .add_enabled(
                parent.is_some(),
                egui::Button::new(ICON_FORMAT_INDENT_DECREASE),
            )
            .on_hover_text("Move out of the container")
            .clicked()
        {
            if let Some(parent) = parent {
                operations.push(GuiBuilderOperation::Move {
                    id,
                    location: GuiLocation::new(parent.parent, parent.index + 1),
                });
            }
        }

        if ui
            .button(ICON_DELETE)
            .on_hover_text("Delete widget")
            .clicked()
        {
            operations.push(GuiBuilderOperation::Remove { id });
            *selected = None;
        }
    });
    operations
}

pub(crate) fn right_sidebar(
    ui: &mut egui::Ui,
    builder: &GuiBuilder,
    selected: Option<Uuid>,
) -> Vec<GuiBuilderOperation> {
    let mut operations = Vec::new();
    ui.heading("Window");
    ui.add_space(4.0);
    let mut title = builder.title().to_owned();
    ui.horizontal(|ui| {
        ui.label("Title");
        if ui.text_edit_singleline(&mut title).changed() {
            operations.push(GuiBuilderOperation::SetTitle { title });
        }
    });
    let mut canvas = builder.canvas();
    let mut resized = false;
    ui.horizontal(|ui| {
        ui.label("Size");
        resized |= ui
            .add(
                egui::DragValue::new(&mut canvas.width)
                    .range(MIN_CANVAS_SIZE..=MAX_CANVAS_SIZE)
                    .speed(1.0),
            )
            .changed();
        resized |= ui
            .add(
                egui::DragValue::new(&mut canvas.height)
                    .range(MIN_CANVAS_SIZE..=MAX_CANVAS_SIZE)
                    .speed(1.0),
            )
            .changed();
    });
    if resized {
        operations.push(GuiBuilderOperation::SetCanvasSize { canvas });
    }

    ui.add_space(8.0);
    ui.separator();
    let Some(widget) = selected.and_then(|id| builder.widget(id)) else {
        ui.heading("Widget");
        ui.add_space(4.0);
        ui.weak("Select a widget to edit its properties.");
        return operations;
    };
    ui.heading(widget.kind.display_name());
    ui.add_space(4.0);
    operations.extend(properties(ui, widget));
    operations
}

fn properties(ui: &mut egui::Ui, widget: &GuiWidget) -> Option<GuiBuilderOperation> {
    let mut kind = widget.kind.clone();
    let mut changed = false;
    match &mut kind {
        GuiWidgetKind::Heading { text }
        | GuiWidgetKind::Label { text }
        | GuiWidgetKind::Button { text } => {
            changed |= field(ui, "Text", |ui| ui.text_edit_singleline(text).changed());
        }
        GuiWidgetKind::TextField {
            label,
            value,
            multiline,
        } => {
            changed |= field(ui, "Label", |ui| ui.text_edit_singleline(label).changed());
            changed |= field(ui, "Value", |ui| ui.text_edit_singleline(value).changed());
            changed |= field(ui, "Multiline", |ui| ui.checkbox(multiline, "").changed());
        }
        GuiWidgetKind::Checkbox { label, checked } => {
            changed |= field(ui, "Label", |ui| ui.text_edit_singleline(label).changed());
            changed |= field(ui, "Checked", |ui| ui.checkbox(checked, "").changed());
        }
        GuiWidgetKind::Slider {
            label,
            value,
            min,
            max,
        } => {
            changed |= field(ui, "Label", |ui| ui.text_edit_singleline(label).changed());
            changed |= field(ui, "Minimum", |ui| {
                ui.add(egui::DragValue::new(min).speed(0.1)).changed()
            });
            changed |= field(ui, "Maximum", |ui| {
                ui.add(egui::DragValue::new(max).speed(0.1)).changed()
            });
            changed |= field(ui, "Value", |ui| {
                ui.add(egui::DragValue::new(value).speed(0.1)).changed()
            });
        }
        GuiWidgetKind::Separator => {
            ui.weak("A separator has no properties.");
        }
        GuiWidgetKind::Space { height } => {
            changed |= field(ui, "Height", |ui| {
                ui.add(
                    egui::DragValue::new(height)
                        .range(0.0..=MAX_SPACE_HEIGHT)
                        .speed(1.0),
                )
                .changed()
            });
        }
        GuiWidgetKind::Container { layout, framed } => {
            changed |= field(ui, "Layout", |ui| {
                let mut changed = false;
                changed |= ui
                    .selectable_value(layout, GuiLayout::Vertical, "Column")
                    .changed();
                changed |= ui
                    .selectable_value(layout, GuiLayout::Horizontal, "Row")
                    .changed();
                changed
            });
            changed |= field(ui, "Framed", |ui| ui.checkbox(framed, "").changed());
        }
    }
    changed.then_some(GuiBuilderOperation::SetKind {
        id: widget.id,
        kind,
    })
}

fn field(ui: &mut egui::Ui, label: &str, content: impl FnOnce(&mut egui::Ui) -> bool) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        content(ui)
    })
    .inner
}
