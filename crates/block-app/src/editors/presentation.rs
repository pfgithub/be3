use std::collections::HashMap;

use block::{Block, BlockParent, BlockReference, BlockReferenceList};
use block_client::{
    blocks::{
        infinite_canvas::{
            CanvasPoint, CanvasPreviewRegion, InfiniteCanvas, InfiniteCanvasOperation,
        },
        presentation::{Presentation, PresentationOperation, PresentationSlide},
        workspace_index::BlockEntry,
    },
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui::{self, Color32, Key, Modifiers, Rect, Sense, Stroke, Vec2};
use egui_material_icons::icons::{
    ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_DELETE, ICON_FULLSCREEN,
    ICON_FULLSCREEN_EXIT, ICON_KEYBOARD_ARROW_DOWN, ICON_SLIDESHOW,
};
use uuid::Uuid;

use crate::block_picker::BlockPicker;

use super::{
    infinite_canvas::InfiniteCanvasEditor, BlockEditor, BlockRenderContext,
    DirectEditorCapabilities, DirectEditorInteraction, DirectEditorResize, DirectEditorViewport,
    EditorAccess, EditorAction, EditorRegistration,
};

const FILMSTRIP_WIDTH: f32 = 210.0;
const THUMBNAIL_SIZE: Vec2 = egui::vec2(176.0, 104.0);
const DEFAULT_SLIDE_SIZE: Vec2 = egui::vec2(960.0, 540.0);
const PLAYBACK_CONTROLS_HEIGHT: f32 = 48.0;

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: Presentation::TYPE_ID,
        display_name: "Presentation",
        icon: ICON_SLIDESHOW,
        create: Some(|client| {
            Box::new(PresentationEditor::new(
                client.create_block(Presentation::new()),
                client,
            ))
        }),
        open: |client, id| {
            Box::new(PresentationEditor::new(
                client.get_block::<Presentation>(id),
                client,
            ))
        },
        can_add_child: true,
        can_delete_child: true,
        dynamic_artifact: None,
    }
}

pub(super) struct PresentationEditor {
    block: BlockHandle<Presentation>,
    dependencies: ReferenceList,
    selected: Option<Uuid>,
    picker_insert_index: Option<usize>,
    picker: BlockPicker,
    dragging: Option<Uuid>,
    presenting: bool,
    active_this_frame: bool,
    last_context: Option<egui::Context>,
    playback_controls_visible_until: f64,
}

impl PresentationEditor {
    fn new(block: BlockHandle<Presentation>, client: &BlockClient) -> Self {
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            dependencies,
            selected: None,
            picker_insert_index: None,
            picker: BlockPicker::default(),
            dragging: None,
            presenting: false,
            active_this_frame: false,
            last_context: None,
            playback_controls_visible_until: 0.0,
        }
    }

    fn slides(&self) -> Option<Vec<PresentationSlide>> {
        self.block
            .read()
            .map(|presentation| presentation.slides().to_vec())
    }

    fn synchronize_selection(&mut self, slides: &[PresentationSlide]) {
        if self
            .selected
            .is_none_or(|selected| !slides.iter().any(|slide| slide.id == selected))
        {
            self.selected = slides.first().map(|slide| slide.id);
        }
    }

    fn selected_slide<'a>(&self, slides: &'a [PresentationSlide]) -> Option<&'a PresentationSlide> {
        let selected = self.selected?;
        slides.iter().find(|slide| slide.id == selected)
    }

    fn dependency_map(&self) -> HashMap<Uuid, BlockReference> {
        self.dependencies
            .read()
            .iter()
            .cloned()
            .map(|reference| (reference.id, reference))
            .collect()
    }

    fn ensure_slide_editors(
        slides: &[PresentationSlide],
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        for slide in slides {
            if let Some(reference) = dependencies.get(&slide.block_id) {
                editors.ensure(reference.id, reference.block_type);
            }
        }
    }

    fn insert_slide(&mut self, block_id: Uuid, index: usize) -> Uuid {
        let slide = PresentationSlide {
            id: Uuid::new_v4(),
            block_id,
        };
        self.block.operate(PresentationOperation::Insert {
            slide: slide.clone(),
            index,
        });
        self.selected = Some(slide.id);
        slide.id
    }

    fn insert_canvas_slide(&mut self, editors: &mut EditorAccess<'_>, index: usize) {
        let mut canvas = InfiniteCanvas::new();
        InfiniteCanvas::apply_operation(
            &mut canvas,
            &InfiniteCanvasOperation::SetPreviewRegion {
                region: Some(CanvasPreviewRegion::new(
                    CanvasPoint::default(),
                    CanvasPoint::new(DEFAULT_SLIDE_SIZE.x, DEFAULT_SLIDE_SIZE.y),
                )),
            },
        );
        let block = editors.client().create_block(canvas);
        let id = block.id();
        block.set_parent(BlockParent::Uuid(self.block.id()));
        let editor = Box::new(InfiniteCanvasEditor::new(block, editors.client()));
        editors.insert(editor);
        self.insert_slide(id, index);
    }

    fn remove_slide(&mut self, slide_id: Uuid) {
        let Some(slides) = self.slides() else {
            return;
        };
        let index = slides.iter().position(|slide| slide.id == slide_id);
        self.block
            .operate(PresentationOperation::Remove { slide_id });
        if self.selected == Some(slide_id) {
            self.selected = index.and_then(|index| {
                slides
                    .get(index + 1)
                    .or_else(|| index.checked_sub(1).and_then(|index| slides.get(index)))
                    .map(|slide| slide.id)
            });
        }
    }

    fn open_add_menu(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) {
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Add slide", ICON_ADD.codepoint))
                .clicked()
            {
                let index = self.slides().map_or(0, |slides| slides.len());
                self.insert_canvas_slide(editors, index);
            }
            ui.menu_button(ICON_KEYBOARD_ARROW_DOWN, |ui| {
                self.picker_insert_index = Some(self.slides().map_or(0, |slides| slides.len()));
                self.picker
                    .show_menu_excluding(ui, editors.registry(), [self.block.id()]);
            })
            .response
            .on_hover_text("More slide options");
        });
    }

    fn show_filmstrip(
        &mut self,
        ui: &mut egui::Ui,
        slides: &[PresentationSlide],
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.open_add_menu(ui, editors);
        ui.separator();
        if slides.is_empty() {
            ui.weak("Add a block to create the first slide.");
            return None;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, slide) in slides.iter().enumerate() {
                let selected = self.selected == Some(slide.id);
                let response = egui::Frame::new()
                    .fill(if selected {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().faint_bg_color
                    })
                    .stroke(Stroke::new(
                        if selected { 2.0_f32 } else { 1.0_f32 },
                        if selected {
                            ui.visuals().selection.stroke.color
                        } else {
                            ui.visuals().widgets.noninteractive.bg_stroke.color
                        },
                    ))
                    .corner_radius(6.0)
                    .inner_margin(6.0)
                    .show(ui, |ui| {
                        let (response, painter) =
                            ui.allocate_painter(THUMBNAIL_SIZE, Sense::click_and_drag());
                        let preview = response.rect.shrink(4.0);
                        let rendered = editors.render(
                            slide.block_id,
                            BlockRenderContext {
                                painter: &painter,
                                corners: rect_corners(preview),
                                opacity: 1.0,
                            },
                        );
                        if !rendered {
                            paint_fallback(
                                &painter,
                                preview,
                                dependencies.get(&slide.block_id),
                                editors,
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.small(format!("{}", index + 1));
                            let name = dependencies
                                .get(&slide.block_id)
                                .map_or("Loadingâ€¦", |reference| reference.name.as_str());
                            ui.add(
                                egui::Label::new(name)
                                    .truncate()
                                    .sense(Sense::click_and_drag()),
                            );
                            if ui
                                .small_button(ICON_DELETE)
                                .on_hover_text("Detach slide")
                                .clicked()
                            {
                                self.remove_slide(slide.id);
                            }
                        });
                        response
                    })
                    .inner;
                if response.clicked() {
                    self.selected = Some(slide.id);
                }
                if response.drag_started() {
                    self.dragging = Some(slide.id);
                }
                if response.hovered()
                    && ui.input(|input| input.pointer.any_released())
                    && self.dragging.is_some_and(|dragging| dragging != slide.id)
                {
                    let dragging = self.dragging.take().unwrap();
                    self.block.operate(PresentationOperation::Move {
                        slide_id: dragging,
                        index,
                    });
                }
                ui.add_space(8.0);
            }
        });
        if ui.input(|input| input.pointer.any_released()) {
            self.dragging = None;
        }
        None
    }

    fn enter_playback(&mut self, context: &egui::Context) {
        self.presenting = true;
        self.last_context = Some(context.clone());
        context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        context.request_repaint();
    }

    fn exit_playback(&mut self) {
        if !std::mem::take(&mut self.presenting) {
            return;
        }
        if let Some(context) = &self.last_context {
            context.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
            context.request_repaint();
        }
    }

    fn navigate_playback(&mut self, slides: &[PresentationSlide], delta: isize) {
        let current = self
            .selected
            .and_then(|selected| slides.iter().position(|slide| slide.id == selected))
            .unwrap_or(0);
        let next = current
            .saturating_add_signed(delta)
            .min(slides.len().saturating_sub(1));
        self.selected = slides.get(next).map(|slide| slide.id);
    }

    fn show_playback_surface(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        slides: &[PresentationSlide],
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
        fullscreen: bool,
    ) -> bool {
        ui.allocate_rect(rect, Sense::hover());
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, Color32::BLACK);
        let Some(slide) = self.selected_slide(slides) else {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "This presentation has no slides",
                egui::FontId::proportional(24.0),
                Color32::WHITE,
            );
            return false;
        };

        let ratio = editors
            .preview_aspect_ratio(slide.block_id)
            .or_else(|| {
                editors
                    .direct_editor_intrinsic_size(slide.block_id)
                    .filter(|size| size.x > 0.0 && size.y > 0.0)
                    .map(|size| size.x / size.y)
            })
            .unwrap_or(DEFAULT_SLIDE_SIZE.x / DEFAULT_SLIDE_SIZE.y);
        let preview = fit_rect(rect, ratio);
        if !editors.render(
            slide.block_id,
            BlockRenderContext {
                painter: &painter,
                corners: rect_corners(preview),
                opacity: 1.0,
            },
        ) {
            paint_fallback(
                &painter,
                preview,
                dependencies.get(&slide.block_id),
                editors,
            );
        }

        let controls_height = PLAYBACK_CONTROLS_HEIGHT.min(rect.height() * 0.3);
        let controls = Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - controls_height),
            rect.max,
        );
        let (now, pointer, pointer_delta) = ui
            .ctx()
            .input(|input| (input.time, input.pointer.hover_pos(), input.pointer.delta()));
        let pointer_moved =
            pointer.is_some_and(|pointer| rect.contains(pointer)) && pointer_delta != Vec2::ZERO;
        let controls_hovered = pointer.is_some_and(|pointer| controls.contains(pointer));
        if pointer_moved || controls_hovered {
            self.playback_controls_visible_until = now + 2.5;
        }
        if now > self.playback_controls_visible_until {
            return false;
        }

        let current = slides
            .iter()
            .position(|candidate| candidate.id == slide.id)
            .unwrap_or(0);
        let mut fullscreen_clicked = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(controls), |ui| {
            egui::Frame::new()
                .fill(Color32::from_black_alpha(180))
                .inner_margin(egui::Margin {
                    left: 12,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    ui.set_min_size(controls.size() - egui::vec2(12.0, 0.0));
                    ui.with_layout(
                        egui::Layout::left_to_right(egui::Align::Center)
                            .with_main_align(egui::Align::Center),
                        |ui| {
                            if ui
                                .add_enabled(current > 0, egui::Button::new(ICON_ARROW_BACK))
                                .on_hover_text("Previous slide")
                                .clicked()
                            {
                                self.navigate_playback(slides, -1);
                            }
                            ui.colored_label(
                                Color32::WHITE,
                                format!("{} / {}", current + 1, slides.len()),
                            );
                            if ui
                                .add_enabled(
                                    current + 1 < slides.len(),
                                    egui::Button::new(ICON_ARROW_FORWARD),
                                )
                                .on_hover_text("Next slide")
                                .clicked()
                            {
                                self.navigate_playback(slides, 1);
                            }
                            let (fullscreen_icon, fullscreen_label) = if fullscreen {
                                (ICON_FULLSCREEN_EXIT, "Exit fullscreen")
                            } else {
                                (ICON_FULLSCREEN, "Enter fullscreen")
                            };
                            if ui
                                .button(fullscreen_icon)
                                .on_hover_text(fullscreen_label)
                                .clicked()
                            {
                                fullscreen_clicked = true;
                            }
                        },
                    );
                });
        });
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        fullscreen_clicked
    }

    fn show_playback(
        &mut self,
        context: &egui::Context,
        slides: &[PresentationSlide],
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        let exit = context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape));
        let previous = context.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::ArrowLeft)
                || input.consume_key(Modifiers::NONE, Key::PageUp)
        });
        let next = context.input_mut(|input| {
            input.consume_key(Modifiers::NONE, Key::ArrowRight)
                || input.consume_key(Modifiers::NONE, Key::Space)
                || input.consume_key(Modifiers::NONE, Key::PageDown)
        });
        if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Home)) {
            self.selected = slides.first().map(|slide| slide.id);
        }
        if context.input_mut(|input| input.consume_key(Modifiers::NONE, Key::End)) {
            self.selected = slides.last().map(|slide| slide.id);
        }
        if previous {
            self.navigate_playback(slides, -1);
        }
        if next {
            self.navigate_playback(slides, 1);
        }
        if exit {
            self.exit_playback();
            return;
        }

        let screen = context.content_rect();
        let mut exit_clicked = false;
        egui::Area::new(egui::Id::new(("presentation-playback", self.block.id())))
            .order(egui::Order::Tooltip)
            .fixed_pos(screen.min)
            .show(context, |ui| {
                ui.set_min_size(screen.size());
                exit_clicked =
                    self.show_playback_surface(ui, screen, slides, dependencies, editors, true);
            });
        if exit_clicked {
            self.exit_playback();
        }
        context.request_repaint();
    }
}

impl BlockEditor for PresentationEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?.slides().len();
        self.block.operate(PresentationOperation::Insert {
            slide: PresentationSlide {
                id: Uuid::new_v4(),
                block_id: entry.id,
            },
            index,
        });
        Some(true)
    }

    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        let slides = self.block.read()?.slides().to_vec();
        let operations = slides
            .iter()
            .filter(|slide| slide.block_id == entry.id)
            .map(|slide| PresentationOperation::Remove { slide_id: slide.id })
            .collect::<Vec<_>>();
        if !operations.is_empty() {
            self.block.operate_grouped(operations);
        }
        Some(true)
    }

    fn set_tab_active(&mut self, active: bool) {
        if active {
            self.active_this_frame = true;
        }
    }

    fn finish_frame(&mut self) {
        if !std::mem::take(&mut self.active_this_frame) {
            self.exit_playback();
        }
    }

    fn tab_closed(&mut self) {
        self.exit_playback();
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let Some(slides) = self.slides() else {
            return false;
        };
        let Some(slide) = slides.first() else {
            return false;
        };
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);
        if editors.render(slide.block_id, BlockRenderContext { ..context }) {
            return true;
        }
        let rect = Rect::from_min_max(context.corners[0], context.corners[2]);
        paint_fallback(
            context.painter,
            rect,
            dependencies.get(&slide.block_id),
            editors,
        );
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: true,
        }
    }

    fn direct_editor_interaction(&self) -> DirectEditorInteraction {
        DirectEditorInteraction::Playback
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        DirectEditorResize::Both
    }

    fn direct_editor_fills_viewport(&self) -> bool {
        true
    }

    fn direct_editor_handles_viewport_input(&self, editors: &EditorAccess<'_>) -> bool {
        let Some(slides) = self.slides() else {
            return false;
        };
        self.selected_slide(&slides)
            .is_some_and(|slide| editors.direct_editor_handles_viewport_input(slide.block_id))
    }

    fn direct_editor_intrinsic_size(&mut self, editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let slides = self.slides()?;
        self.synchronize_selection(&slides);
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);
        self.selected_slide(&slides)
            .and_then(|slide| editors.direct_editor_intrinsic_size(slide.block_id))
            .or(Some(DEFAULT_SLIDE_SIZE))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let slides = self.slides()?;
        self.synchronize_selection(&slides);
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);
        self.open_add_menu(ui, editors);
        if ui
            .add_enabled(!slides.is_empty(), egui::Button::new(ICON_FULLSCREEN))
            .on_hover_text("Present")
            .clicked()
        {
            self.enter_playback(ui.ctx());
        }
        let mut action = None;
        if let Some(slide) = self.selected_slide(&slides) {
            ui.separator();
            action = action.or_else(|| editors.direct_editor_top_bar(slide.block_id, ui, viewport));
        }
        action
    }

    fn direct_editor_has_left_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        ui.set_width(FILMSTRIP_WIDTH);
        let Some(slides) = self.slides() else {
            ui.spinner();
            return None;
        };
        self.synchronize_selection(&slides);
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);
        self.show_filmstrip(ui, &slides, &dependencies, editors)
    }

    fn direct_editor_has_right_sidebar(&self, editors: &mut EditorAccess<'_>) -> bool {
        let Some(slides) = self.slides() else {
            return false;
        };
        self.selected_slide(&slides)
            .is_some_and(|slide| editors.direct_editor_has_right_sidebar(slide.block_id))
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let slides = self.slides()?;
        let slide = self.selected_slide(&slides)?;
        editors.direct_editor_right_sidebar(slide.block_id, ui)
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.active_this_frame = true;
        self.last_context = Some(ui.ctx().clone());
        let Some(slides) = self.slides() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        self.synchronize_selection(&slides);
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);

        if let Some(result) =
            self.picker
                .handle(ui.ctx(), editors, BlockParent::Uuid(self.block.id()))
        {
            let index = self.picker_insert_index.take().unwrap_or(slides.len());
            self.insert_slide(result.id, index);
        }
        if self.presenting {
            self.show_playback(ui.ctx(), &slides, &dependencies, editors);
        }
        let Some(slide) = self.selected_slide(&slides) else {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("Empty presentation");
                    ui.weak("Use Add slide to create or link a block.");
                });
            });
            return None;
        };

        let mut action = None;
        if dependencies
            .get(&slide.block_id)
            .is_some_and(|reference| reference.block_type == InfiniteCanvas::TYPE_ID)
        {
            viewport.auto_fit(slide.id);
        }
        if editors.direct_editor_has_left_sidebar(slide.block_id) {
            egui::Panel::left(egui::Id::new((
                "presentation-slide-left",
                self.block.id(),
                slide.id,
            )))
            .default_size(220.0)
            .show_inside(ui, |ui| {
                action = editors.direct_editor_left_sidebar(slide.block_id, ui);
            });
        }
        action.or_else(|| editors.direct_editor_ui(slide.block_id, ui, scale, viewport))
    }

    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.active_this_frame = true;
        self.last_context = Some(ui.ctx().clone());
        let rect = ui.available_rect_before_wrap();
        let Some(slides) = self.slides() else {
            ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "Loading presentationâ€¦",
                egui::FontId::proportional(18.0),
                Color32::WHITE,
            );
            return None;
        };
        self.synchronize_selection(&slides);
        let dependencies = self.dependency_map();
        Self::ensure_slide_editors(&slides, &dependencies, editors);
        if self.presenting {
            self.show_playback(ui.ctx(), &slides, &dependencies, editors);
        }
        if self.show_playback_surface(ui, rect, &slides, &dependencies, editors, false) {
            self.enter_playback(ui.ctx());
        }
        None
    }
}

fn rect_corners(rect: Rect) -> [egui::Pos2; 4] {
    [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
}

fn fit_rect(available: Rect, ratio: f32) -> Rect {
    let ratio = ratio.max(0.01);
    let available_ratio = available.width() / available.height().max(1.0);
    let size = if available_ratio > ratio {
        Vec2::new(available.height() * ratio, available.height())
    } else {
        Vec2::new(available.width(), available.width() / ratio)
    };
    Rect::from_center_size(available.center(), size)
}

fn paint_fallback(
    painter: &egui::Painter,
    rect: Rect,
    reference: Option<&BlockReference>,
    editors: &EditorAccess<'_>,
) {
    painter.rect_filled(rect, 5.0, Color32::from_gray(28));
    painter.rect_stroke(
        rect,
        5.0,
        Stroke::new(1.0_f32, Color32::from_gray(75)),
        egui::StrokeKind::Inside,
    );
    let (name, icon) = reference.map_or(("Loadingâ€¦", None), |reference| {
        (
            if reference.name.is_empty() {
                "Untitled"
            } else {
                reference.name.as_str()
            },
            editors.registry().icon(reference.block_type),
        )
    });
    let center = rect.center();
    if let Some(icon) = icon {
        painter.text(
            center - Vec2::new(0.0, 18.0),
            egui::Align2::CENTER_CENTER,
            icon.codepoint,
            egui::FontId::new(28.0, icon.font_family()),
            Color32::LIGHT_GRAY,
        );
    }
    painter.text(
        center + Vec2::new(0.0, 18.0),
        egui::Align2::CENTER_CENTER,
        name,
        egui::FontId::proportional(16.0),
        Color32::LIGHT_GRAY,
    );
}
