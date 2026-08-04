mod inspector;
mod surface;

use block::Block;
use block_client::{
    blocks::gui_builder::{
        GuiBuilder, GuiBuilderOperation, GuiCanvasSize, GuiLayout, GuiLocation, GuiWidgetKind,
    },
    BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{
        ICON_CHECK_BOX, ICON_CODE, ICON_CONTENT_COPY, ICON_DESIGN_SERVICES, ICON_HORIZONTAL_RULE,
        ICON_LABEL, ICON_PLAY_ARROW, ICON_SMART_BUTTON, ICON_SPACE_BAR, ICON_TEXT_FIELDS,
        ICON_TITLE, ICON_TUNE, ICON_VIEW_COLUMN, ICON_VIEW_STREAM, ICON_WIDGETS,
    },
    MaterialIcon,
};
use uuid::Uuid;

use self::surface::{PreviewState, Surface};

use super::{
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorInteraction,
    DirectEditorResize, DirectEditorViewport, EditorAccess, EditorAction, EditorRegistration,
};

const TITLE_BAR_HEIGHT: f32 = 26.0;
const CANVAS_PADDING: f32 = 8.0;
const CANVAS_CORNER_RADIUS: f32 = 6.0;
const PREVIEW_LINE_HEIGHT: f32 = 14.0;

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: GuiBuilder::TYPE_ID,
        display_name: "GUI Builder",
        icon: ICON_WIDGETS,
        create: Some(|client| {
            Box::new(GuiBuilderEditor::new(
                client.create_block(GuiBuilder::new()),
            ))
        }),
        open: |client, id| Box::new(GuiBuilderEditor::new(client.get_block::<GuiBuilder>(id))),
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

pub(super) struct GuiBuilderEditor {
    block: BlockHandle<GuiBuilder>,
    /// Design mode edits the widget tree; preview mode runs it.
    design: bool,
    selected: Option<Uuid>,
    preview: PreviewState,
    show_code: bool,
}

impl GuiBuilderEditor {
    fn new(block: BlockHandle<GuiBuilder>) -> Self {
        Self {
            block,
            design: true,
            selected: None,
            preview: PreviewState::default(),
            show_code: false,
        }
    }

    fn builder(&self) -> Option<GuiBuilder> {
        self.block.read().map(|builder| builder.clone())
    }

    fn apply(&self, operations: Vec<GuiBuilderOperation>) {
        if !operations.is_empty() {
            self.block.operate_grouped(operations);
        }
    }

    /// Keeps the selection pointing at a widget that still exists.
    fn synchronize_selection(&mut self, builder: &GuiBuilder) {
        if self.selected.is_some_and(|id| builder.widget(id).is_none()) {
            self.selected = None;
        }
    }

    fn show_code_window(&mut self, ui: &egui::Ui, builder: &GuiBuilder) {
        if !self.show_code {
            return;
        }
        let code = builder.generate_code();
        let mut open = true;
        egui::Window::new("Generated code")
            .id(egui::Id::new(("gui-builder-code", self.block.id())))
            .open(&mut open)
            .default_width(520.0)
            .default_height(420.0)
            .vscroll(false)
            .show(ui.ctx(), |ui| {
                if ui
                    .button(format!("{} Copy", ICON_CONTENT_COPY.codepoint))
                    .clicked()
                {
                    ui.ctx().copy_text(code.clone());
                }
                ui.add_space(4.0);
                egui::ScrollArea::both().show(ui, |ui| {
                    // Read-only: the design is the source of truth, so the
                    // code is shown rather than edited.
                    ui.add(
                        egui::TextEdit::multiline(&mut code.as_str())
                            .code_editor()
                            .desired_width(f32::INFINITY),
                    );
                });
            });
        self.show_code = open;
    }

    fn show_window(&mut self, ui: &mut egui::Ui, builder: &GuiBuilder) {
        let canvas = builder.canvas();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(canvas.width, canvas.height),
            egui::Sense::hover(),
        );
        let visuals = ui.visuals();
        ui.painter().rect(
            rect,
            CANVAS_CORNER_RADIUS,
            visuals.panel_fill,
            visuals.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let title_rect = egui::Rect::from_min_size(
            rect.min,
            egui::vec2(rect.width(), TITLE_BAR_HEIGHT.min(rect.height())),
        );
        ui.painter()
            .rect_filled(title_rect, CANVAS_CORNER_RADIUS, visuals.faint_bg_color);
        ui.painter().with_clip_rect(title_rect).text(
            title_rect.left_center() + egui::vec2(CANVAS_PADDING, 0.0),
            egui::Align2::LEFT_CENTER,
            builder.title(),
            egui::FontId::proportional(13.0),
            visuals.strong_text_color(),
        );

        let body = egui::Rect::from_min_max(egui::pos2(rect.left(), title_rect.bottom()), rect.max)
            .shrink(CANVAS_PADDING);
        if body.width() <= 0.0 || body.height() <= 0.0 {
            return;
        }
        let mut content = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("gui-builder-surface", self.block.id()))
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        content.set_clip_rect(body.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(("gui-builder-scroll", self.block.id()))
            .auto_shrink([false, false])
            .show(&mut content, |ui| {
                if builder.widgets().is_empty() {
                    ui.weak("Add widgets from the palette on the left.");
                    return;
                }
                Surface {
                    design: self.design,
                    state: &mut self.preview,
                    selected: &mut self.selected,
                }
                .show(ui, builder.widgets());
            });
    }
}

impl BlockEditor for GuiBuilderEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn render(&mut self, context: BlockRenderContext<'_>, _editors: &mut EditorAccess<'_>) -> bool {
        let Some(builder) = self.builder() else {
            return false;
        };
        let rect = egui::Rect::from_min_max(context.corners[0], context.corners[2]);
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return false;
        }
        let painter = context.painter;
        let faint = egui::Color32::from_gray(96).gamma_multiply(context.opacity);
        let strong = egui::Color32::from_gray(200).gamma_multiply(context.opacity);
        painter.rect_filled(
            rect,
            CANVAS_CORNER_RADIUS,
            egui::Color32::from_gray(32).gamma_multiply(context.opacity),
        );
        let title_height = TITLE_BAR_HEIGHT.min(rect.height());
        let title_rect =
            egui::Rect::from_min_size(rect.min, egui::vec2(rect.width(), title_height));
        painter.rect_filled(title_rect, CANVAS_CORNER_RADIUS, faint);
        painter.with_clip_rect(title_rect).text(
            title_rect.left_center() + egui::vec2(CANVAS_PADDING, 0.0),
            egui::Align2::LEFT_CENTER,
            builder.title(),
            egui::FontId::proportional(13.0),
            strong,
        );
        // A thumbnail only needs the shape of the design, so each top-level
        // widget becomes one line.
        let mut top = title_rect.bottom() + CANVAS_PADDING;
        for widget in builder.widgets() {
            if top + PREVIEW_LINE_HEIGHT > rect.bottom() {
                break;
            }
            let line = egui::Rect::from_min_size(
                egui::pos2(rect.left() + CANVAS_PADDING, top),
                egui::vec2(
                    (rect.width() - CANVAS_PADDING * 2.0).max(1.0),
                    PREVIEW_LINE_HEIGHT,
                ),
            );
            painter.rect_filled(line, 2.0, faint);
            painter.with_clip_rect(line).text(
                line.left_center() + egui::vec2(4.0, 0.0),
                egui::Align2::LEFT_CENTER,
                widget.kind.display_name(),
                egui::FontId::proportional(10.0),
                strong,
            );
            top += PREVIEW_LINE_HEIGHT + 4.0;
        }
        true
    }

    fn render_aspect_ratio(&self) -> Option<f32> {
        let canvas = self.block.read()?.canvas();
        Some(canvas.width / canvas.height)
    }

    fn default_preserve_aspect_ratio(&self) -> bool {
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        DirectEditorInteraction::Live
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        DirectEditorResize::Both
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let canvas = self.block.read()?.canvas();
        Some(egui::vec2(canvas.width, canvas.height))
    }

    fn set_direct_editor_intrinsic_size(
        &mut self,
        size: egui::Vec2,
        _editors: &mut EditorAccess<'_>,
    ) -> bool {
        let canvas = GuiCanvasSize::new(size.x, size.y);
        if self.block.read().is_none_or(|builder| {
            let current = builder.canvas();
            (current.width - canvas.width).abs() < 0.5
                && (current.height - canvas.height).abs() < 0.5
        }) {
            return false;
        }
        self.block
            .operate(GuiBuilderOperation::SetCanvasSize { canvas });
        true
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        ui.horizontal(|ui| {
            if ui
                .selectable_label(
                    self.design,
                    format!("{} Design", ICON_DESIGN_SERVICES.codepoint),
                )
                .clicked()
            {
                self.design = true;
            }
            if ui
                .selectable_label(
                    !self.design,
                    format!("{} Preview", ICON_PLAY_ARROW.codepoint),
                )
                .clicked()
            {
                // Each preview run starts from the designed values.
                self.design = false;
                self.preview.reset();
            }
            ui.separator();
            if ui
                .selectable_label(self.show_code, format!("{} Code", ICON_CODE.codepoint))
                .clicked()
            {
                self.show_code = !self.show_code;
            }
        });
        None
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.design
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let builder = self.builder()?;
        self.synchronize_selection(&builder);
        let operations = inspector::left_sidebar(ui, &builder, &mut self.selected);
        self.apply(operations);
        None
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        self.design
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let builder = self.builder()?;
        self.synchronize_selection(&builder);
        let operations = inspector::right_sidebar(ui, &builder, self.selected);
        self.apply(operations);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(builder) = self.builder() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        self.synchronize_selection(&builder);
        self.show_code_window(ui, &builder);
        self.show_window(ui, &builder);
        None
    }
}

/// The insertion point for a new widget: inside the selected container, or
/// after the selected widget, falling back to the end of the design.
fn insertion_location(builder: &GuiBuilder, selected: Option<Uuid>) -> GuiLocation {
    let fallback = GuiLocation::new(None, builder.widgets().len());
    let Some(id) = selected else {
        return fallback;
    };
    let Some(widget) = builder.widget(id) else {
        return fallback;
    };
    if widget.kind.is_container() {
        return GuiLocation::new(Some(id), widget.children.len());
    }
    builder.location(id).map_or(fallback, |location| {
        GuiLocation::new(location.parent, location.index + 1)
    })
}

fn widget_icon(kind: &GuiWidgetKind) -> MaterialIcon {
    match kind {
        GuiWidgetKind::Heading { .. } => ICON_TITLE,
        GuiWidgetKind::Label { .. } => ICON_LABEL,
        GuiWidgetKind::Button { .. } => ICON_SMART_BUTTON,
        GuiWidgetKind::TextField { .. } => ICON_TEXT_FIELDS,
        GuiWidgetKind::Checkbox { .. } => ICON_CHECK_BOX,
        GuiWidgetKind::Slider { .. } => ICON_TUNE,
        GuiWidgetKind::Separator => ICON_HORIZONTAL_RULE,
        GuiWidgetKind::Space { .. } => ICON_SPACE_BAR,
        GuiWidgetKind::Container {
            layout: GuiLayout::Vertical,
            ..
        } => ICON_VIEW_STREAM,
        GuiWidgetKind::Container {
            layout: GuiLayout::Horizontal,
            ..
        } => ICON_VIEW_COLUMN,
    }
}
