use std::sync::Arc;

use block::Block;
use block_client::blocks::gui_builder::{
    GuiBuilder, GuiBuilderOperation, GuiCanvasSize, GuiLayout, GuiLocation, GuiWidgetKind,
};
use block_client::blocks::text::TextDocument;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_CHECK_BOX, ICON_CODE, ICON_DESIGN_SERVICES, ICON_HORIZONTAL_RULE, ICON_LABEL,
    ICON_PLAY_ARROW, ICON_SMART_BUTTON, ICON_SPACE_BAR, ICON_TEXT_FIELDS, ICON_TITLE, ICON_TUNE,
    ICON_VIEW_COLUMN, ICON_VIEW_STREAM,
};
use block_editor_plugin::egui_material_icons::MaterialIcon;
use block_editor_plugin::{egui, Artifact, ArtifactDescription, EditorHost, EditorRegion};
use uuid::Uuid;

use crate::artifact;
use crate::inspector;
use crate::surface::{PreviewState, Surface};

const TITLE_BAR_HEIGHT: f32 = 26.0;
const CANVAS_PADDING: f32 = 8.0;
const CANVAS_CORNER_RADIUS: f32 = 6.0;
const PREVIEW_LINE_HEIGHT: f32 = 14.0;
const DISPLAY_NAME: &str = "GUI Builder";

struct Editing {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<GuiBuilder>,
}

struct Exporting {
    client: Arc<BlockClient>,
    block_id: Uuid,
    block_type: Uuid,
    regeneration: Option<artifact::Regeneration>,
    failure: Option<String>,
}

pub struct GuiBuilderApp {
    editing: Option<Editing>,
    creation: Option<Arc<BlockClient>>,
    exporting: Option<Exporting>,
    design: bool,
    selected: Option<Uuid>,
    preview: PreviewState,
}

impl Default for GuiBuilderApp {
    fn default() -> Self {
        Self {
            editing: None,
            creation: None,
            exporting: None,
            design: true,
            selected: None,
            preview: PreviewState::default(),
        }
    }
}

impl GuiBuilderApp {
    fn builder(&self) -> Option<GuiBuilder> {
        self.editing
            .as_ref()?
            .block
            .read()
            .map(|builder| builder.clone())
    }

    fn apply(&self, operations: Vec<GuiBuilderOperation>) {
        if operations.is_empty() {
            return;
        }
        if let Some(editing) = &self.editing {
            editing.block.operate_grouped(operations);
        }
    }

    fn synchronize_selection(&mut self, builder: &GuiBuilder) {
        if self.selected.is_some_and(|id| builder.widget(id).is_none()) {
            self.selected = None;
        }
    }

    fn export_code(&self) {
        let Some(editing) = &self.editing else {
            return;
        };
        let Some(builder) = editing.block.read() else {
            return;
        };
        let generated = artifact::generate_initial(&builder);
        drop(builder);
        let created = editing
            .client
            .create_dynamic_artifact(generated, artifact::descriptor(editing.block.id()));
        let source_name = editing
            .block
            .name()
            .unwrap_or_else(|| DISPLAY_NAME.to_owned());
        created.set_name(artifact::artifact_name(&source_name));
        editing.host.open_block(created.id(), TextDocument::TYPE_ID);
    }

    fn show_window(&mut self, ui: &mut egui::Ui, builder: &GuiBuilder) {
        let Some(block_id) = self.editing.as_ref().map(|editing| editing.block.id()) else {
            return;
        };
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
                .id_salt(("gui-builder-surface", block_id))
                .max_rect(body)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        content.set_clip_rect(body.intersect(ui.clip_rect()));
        egui::ScrollArea::vertical()
            .id_salt(("gui-builder-scroll", block_id))
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

impl block_editor_plugin::App for GuiBuilderApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.editing = Some(Editing {
            host,
            block: client.get_block(block_id),
            client,
        });
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(GuiBuilder::new()).id())
    }

    fn connect_artifact(
        &mut self,
        _host: EditorHost,
        client: Arc<BlockClient>,
        artifact: Artifact,
    ) {
        self.exporting = Some(Exporting {
            client,
            block_id: artifact.block_id,
            block_type: artifact.block_type,
            regeneration: None,
            failure: None,
        });
    }

    fn describe_artifact(&mut self, data: &[u8]) -> Result<ArtifactDescription, String> {
        artifact::describe(data)
    }

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        artifact::settings_ui(ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(exporting) = &mut self.exporting else {
            return;
        };
        match artifact::Regeneration::start(
            &exporting.client,
            exporting.block_id,
            exporting.block_type,
            data,
        ) {
            Ok(regeneration) => {
                exporting.regeneration = Some(regeneration);
                exporting.failure = None;
            }
            Err(error) => {
                exporting.regeneration = None;
                exporting.failure = Some(error);
            }
        }
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        let exporting = self.exporting.as_mut()?;
        if let Some(failure) = exporting.failure.take() {
            return Some(Err(failure));
        }
        let result = exporting.regeneration.as_mut()?.poll()?;
        exporting.regeneration = None;
        Some(result)
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let canvas = self.editing.as_ref()?.block.read()?.canvas();
        Some(egui::vec2(canvas.width, canvas.height))
    }

    fn set_intrinsic_size(&mut self, size: egui::Vec2) {
        let Some(editing) = &self.editing else {
            return;
        };
        let canvas = GuiCanvasSize::new(size.x, size.y);
        let unchanged = editing.block.read().is_none_or(|builder| {
            let current = builder.canvas();
            (current.width - canvas.width).abs() < 0.5
                && (current.height - canvas.height).abs() < 0.5
        });
        if unchanged {
            return;
        }
        editing
            .block
            .operate(GuiBuilderOperation::SetCanvasSize { canvas });
    }

    fn aspect_ratio(&mut self) -> Option<f32> {
        let canvas = self.editing.as_ref()?.block.read()?.canvas();
        Some(canvas.width / canvas.height)
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let export = ui
            .horizontal(|ui| {
                if ui
                    .selectable_label(
                        self.design,
                        format!("{} Design", ICON_DESIGN_SERVICES.codepoint),
                    )
                    .test_id("gui-builder.design")
                    .clicked()
                {
                    self.design = true;
                }
                if ui
                    .selectable_label(
                        !self.design,
                        format!("{} Preview", ICON_PLAY_ARROW.codepoint),
                    )
                    .test_id("gui-builder.preview")
                    .clicked()
                {
                    self.design = false;
                    self.preview.reset();
                }
                ui.separator();
                ui.button(format!("{} Generate code", ICON_CODE.codepoint))
                    .on_hover_text("Create a Text block holding the generated egui code")
                    .test_id("gui-builder.generate")
                    .clicked()
            })
            .inner;
        if export {
            self.export_code();
        }
        if let Some(editing) = &self.editing {
            editing
                .host
                .show_region(EditorRegion::LeftSidebar, self.design);
            editing
                .host
                .show_region(EditorRegion::RightSidebar, self.design);
        }
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(builder) = self.builder() else {
            return;
        };
        self.synchronize_selection(&builder);
        let operations = inspector::left_sidebar(ui, &builder, &mut self.selected);
        self.apply(operations);
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(builder) = self.builder() else {
            return;
        };
        self.synchronize_selection(&builder);
        let operations = inspector::right_sidebar(ui, &builder, self.selected);
        self.apply(operations);
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let Some(builder) = self.builder() else {
            return;
        };
        let rect = ui.max_rect();
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let painter = ui.painter();
        let faint = egui::Color32::from_gray(96);
        let strong = egui::Color32::from_gray(200);
        painter.rect_filled(rect, CANVAS_CORNER_RADIUS, egui::Color32::from_gray(32));
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
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(builder) = self.builder() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        self.synchronize_selection(&builder);
        self.show_window(ui, &builder);
    }
}

pub(crate) fn insertion_location(builder: &GuiBuilder, selected: Option<Uuid>) -> GuiLocation {
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

pub(crate) fn widget_icon(kind: &GuiWidgetKind) -> MaterialIcon {
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
