use std::collections::HashMap;

use block_client::blocks::gui_builder::{GuiLayout, GuiWidget, GuiWidgetKind};
use block_editor_plugin::egui;
use uuid::Uuid;

use crate::app::widget_icon;

const SELECTION_PADDING: f32 = 1.0;
const MIN_PLACEHOLDER_HEIGHT: f32 = 6.0;

#[derive(Default)]
pub(crate) struct PreviewState {
    texts: HashMap<Uuid, String>,
    flags: HashMap<Uuid, bool>,
    numbers: HashMap<Uuid, f32>,
}

impl PreviewState {
    pub(crate) fn reset(&mut self) {
        self.texts.clear();
        self.flags.clear();
        self.numbers.clear();
    }

    fn text(&mut self, id: Uuid, initial: &str) -> &mut String {
        self.texts.entry(id).or_insert_with(|| initial.to_owned())
    }

    fn flag(&mut self, id: Uuid, initial: bool) -> &mut bool {
        self.flags.entry(id).or_insert(initial)
    }

    fn number(&mut self, id: Uuid, initial: f32) -> &mut f32 {
        self.numbers.entry(id).or_insert(initial)
    }
}

pub(crate) struct Surface<'a> {
    pub(crate) design: bool,
    pub(crate) state: &'a mut PreviewState,
    pub(crate) selected: &'a mut Option<Uuid>,
}

impl Surface<'_> {
    pub(crate) fn show(&mut self, ui: &mut egui::Ui, widgets: &[GuiWidget]) {
        for widget in widgets {
            self.show_widget(ui, widget);
        }
    }

    fn show_widget(&mut self, ui: &mut egui::Ui, widget: &GuiWidget) {
        if let GuiWidgetKind::Container { layout, framed } = widget.kind {
            self.show_container(ui, widget, layout, framed);
            return;
        }
        let design = self.design;
        let rect = ui
            .scope(|ui| {
                if design {
                    ui.style_mut().interaction.selectable_labels = false;
                }
                self.show_leaf(ui, widget);
            })
            .response
            .rect;
        if design {
            self.select(ui, widget.id, rect);
        }
    }

    fn show_leaf(&mut self, ui: &mut egui::Ui, widget: &GuiWidget) {
        let design = self.design;
        let id = widget.id;
        match &widget.kind {
            GuiWidgetKind::Heading { text } => {
                ui.heading(text);
            }
            GuiWidgetKind::Label { text } => {
                ui.label(text);
            }
            GuiWidgetKind::Button { text } => {
                ui.add_enabled(!design, egui::Button::new(text));
            }
            GuiWidgetKind::TextField {
                label,
                value,
                multiline,
            } => {
                if design {
                    let mut shown = value.clone();
                    ui.add_enabled_ui(false, |ui| {
                        text_field(ui, label, *multiline, &mut shown);
                    });
                } else {
                    let value = self.state.text(id, value);
                    text_field(ui, label, *multiline, value);
                }
            }
            GuiWidgetKind::Checkbox { label, checked } => {
                if design {
                    let mut shown = *checked;
                    ui.add_enabled_ui(false, |ui| {
                        ui.checkbox(&mut shown, label);
                    });
                } else {
                    let checked = self.state.flag(id, *checked);
                    ui.checkbox(checked, label);
                }
            }
            GuiWidgetKind::Slider {
                label,
                value,
                min,
                max,
            } => {
                let range = *min..=*max;
                if design {
                    let mut shown = *value;
                    ui.add_enabled_ui(false, |ui| {
                        ui.add(egui::Slider::new(&mut shown, range).text(label));
                    });
                } else {
                    let value = self.state.number(id, *value);
                    ui.add(egui::Slider::new(value, range).text(label));
                }
            }
            GuiWidgetKind::Separator => {
                ui.separator();
            }
            GuiWidgetKind::Space { height } => {
                if design {
                    let size = egui::vec2(
                        ui.available_width().max(1.0),
                        height.max(MIN_PLACEHOLDER_HEIGHT),
                    );
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, 2.0, ui.visuals().faint_bg_color);
                } else {
                    ui.add_space(*height);
                }
            }
            GuiWidgetKind::Container { .. } => {}
        }
    }

    fn show_container(
        &mut self,
        ui: &mut egui::Ui,
        widget: &GuiWidget,
        layout: GuiLayout,
        framed: bool,
    ) {
        let selected = *self.selected == Some(widget.id);
        let mut frame = if framed {
            egui::Frame::group(ui.style())
        } else {
            egui::Frame::new().inner_margin(2.0)
        };
        if self.design {
            frame = frame
                .inner_margin(4.0)
                .corner_radius(4.0)
                .stroke(egui::Stroke::new(
                    if selected { 1.5_f32 } else { 1.0_f32 },
                    if selected {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().weak_text_color()
                    },
                ));
        }
        frame.show(ui, |ui| {
            if self.design {
                let header = egui::RichText::new(format!(
                    "{} {}",
                    widget_icon(&widget.kind).codepoint,
                    widget.kind.summary()
                ))
                .small()
                .weak();
                if ui.selectable_label(selected, header).clicked() {
                    *self.selected = Some(widget.id);
                }
            }
            if widget.children.is_empty() {
                if self.design {
                    ui.weak("Empty");
                }
                return;
            }
            match layout {
                GuiLayout::Vertical => self.show(ui, &widget.children),
                GuiLayout::Horizontal => {
                    ui.horizontal(|ui| self.show(ui, &widget.children));
                }
            }
        });
    }

    fn select(&mut self, ui: &mut egui::Ui, id: Uuid, rect: egui::Rect) {
        let rect = rect.expand(SELECTION_PADDING);
        let response = ui.interact(
            rect,
            egui::Id::new(("gui-builder-widget", id)),
            egui::Sense::click(),
        );
        if response.clicked() {
            *self.selected = Some(id);
        }
        let stroke = if *self.selected == Some(id) {
            egui::Stroke::new(1.5_f32, ui.visuals().selection.stroke.color)
        } else if response.hovered() {
            egui::Stroke::new(1.0_f32, ui.visuals().weak_text_color())
        } else {
            return;
        };
        ui.painter()
            .rect_stroke(rect, 3.0, stroke, egui::StrokeKind::Outside);
    }
}

fn text_field(ui: &mut egui::Ui, label: &str, multiline: bool, value: &mut String) {
    if multiline {
        if !label.is_empty() {
            ui.label(label);
        }
        ui.text_edit_multiline(value);
        return;
    }
    if label.is_empty() {
        ui.text_edit_singleline(value);
        return;
    }
    ui.horizontal(|ui| {
        ui.label(label);
        ui.text_edit_singleline(value);
    });
}
