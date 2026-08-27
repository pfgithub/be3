use std::{collections::HashMap, sync::Arc};

use block::{BlockReference, BlockReferenceList};
use block_client::{
    blocks::presentation::{Presentation, PresentationOperation, PresentationSlide},
    references::{ReferenceClassificationQueue, ReferenceResolutionCache},
    BlockClient, BlockHandle, ReferenceList,
};
use block_editor_plugin::{
    block_ui::{BlockLabel, BlockTypes},
    egui,
    egui_material_icons::icons::{
        ICON_ADD, ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_CLOSE, ICON_DELETE, ICON_FULLSCREEN,
    },
    BlockFilter, BlockPicker, ChildHandle, ChildMode, ChildPart, EditorHost, EditorRegion,
};
use uuid::Uuid;

const FILMSTRIP_WIDTH: f32 = 210.0;
const SLIDE_SIDEBAR_WIDTH: f32 = 240.0;
const THUMBNAIL_SIZE: egui::Vec2 = egui::vec2(176.0, 104.0);
const DEFAULT_SLIDE_SIZE: egui::Vec2 = egui::vec2(960.0, 540.0);
const PLAYBACK_CONTROLS_HEIGHT: f32 = 48.0;
const CONTROLS_VISIBLE_SECONDS: f64 = 2.5;

struct Editor {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<Presentation>,
    dependencies: ReferenceList,
}

struct Slide {
    id: Uuid,
    block_id: Option<Uuid>,
    reference: Option<BlockReference>,
}

#[derive(Default)]
pub struct PresentationApp {
    editor: Option<Editor>,
    creation: Option<Arc<BlockClient>>,
    selected: Option<Uuid>,
    dragging: Option<Uuid>,
    picker: BlockPicker,
    picker_index: Option<usize>,
    controls_visible_until: f64,
    shapes: HashMap<Uuid, f32>,
    sidebars: HashMap<Uuid, (bool, bool)>,
    slide_toolbar: f32,
    reference_cache: ReferenceResolutionCache,
    pending_slides: ReferenceClassificationQueue<(Uuid, usize)>,
}

impl PresentationApp {
    fn host(&self) -> Option<EditorHost> {
        Some(self.editor.as_ref()?.host.clone())
    }

    fn poll(&mut self) {
        self.reference_cache.poll();
        let Some(editor) = &self.editor else {
            return;
        };
        for (block_id, (slide_id, index)) in self.pending_slides.poll() {
            editor.block.operate(PresentationOperation::Insert {
                slide: PresentationSlide {
                    id: slide_id,
                    block_id,
                },
                index,
            });
        }
    }

    fn slides(&mut self) -> Option<Vec<Slide>> {
        let editor = self.editor.as_ref()?;
        let presentation = editor.block.read()?;
        let entries = presentation.slides().to_vec();
        drop(presentation);
        let metadata: HashMap<_, _> = editor
            .dependencies
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let client = Arc::clone(&editor.client);
        let referencing_id = editor.block.id();
        let cache = &mut self.reference_cache;
        let slides: Vec<_> = entries
            .into_iter()
            .map(|slide| {
                let block_id = cache.resolve(&client, referencing_id, slide.block_id);
                Slide {
                    id: slide.id,
                    block_id,
                    reference: block_id.and_then(|id| metadata.get(&id).cloned()),
                }
            })
            .collect();
        if self
            .selected
            .is_none_or(|selected| !slides.iter().any(|slide| slide.id == selected))
        {
            self.selected = slides.first().map(|slide| slide.id);
        }
        Some(slides)
    }

    fn selected_index(&self, slides: &[Slide]) -> Option<usize> {
        let selected = self.selected?;
        slides.iter().position(|slide| slide.id == selected)
    }

    fn select(&mut self, slides: &[Slide], index: usize) {
        self.selected = slides
            .get(index.min(slides.len().saturating_sub(1)))
            .map(|slide| slide.id);
    }

    fn open_picker(&mut self, index: usize) {
        let Some(host) = self.host() else {
            return;
        };
        self.picker_index = Some(index);
        self.picker.open(
            &host,
            BlockFilter {
                name: "Slide".into(),
                block_types: Vec::new(),
                templates: true,
            },
        );
    }

    fn poll_picker(&mut self, count: usize) {
        let Some(host) = self.host() else {
            return;
        };
        let Some(picked) = self.picker.poll(&host) else {
            return;
        };
        let index = self.picker_index.take().unwrap_or(count);
        let Ok((block_id, _)) = picked else {
            return;
        };
        let Some(editor) = &self.editor else {
            return;
        };
        let slide_id = Uuid::new_v4();
        let client = Arc::clone(&editor.client);
        let referencing_id = editor.block.id();
        self.pending_slides
            .push(&client, referencing_id, block_id, (slide_id, index));
        self.selected = Some(slide_id);
    }

    fn operate(&self, operation: PresentationOperation) {
        if let Some(editor) = &self.editor {
            editor.block.operate(operation);
        }
    }

    fn remove_slide(&mut self, slides: &[Slide], slide_id: Uuid) {
        let index = slides.iter().position(|slide| slide.id == slide_id);
        self.operate(PresentationOperation::Remove { slide_id });
        if self.selected == Some(slide_id) {
            self.selected = index.and_then(|index| {
                slides
                    .get(index + 1)
                    .or_else(|| index.checked_sub(1).and_then(|index| slides.get(index)))
                    .map(|slide| slide.id)
            });
        }
    }

    fn place_slide(
        &mut self,
        ui: &mut egui::Ui,
        slide: &Slide,
        size: Option<egui::Vec2>,
    ) -> Option<ChildHandle> {
        let host = self.host()?;
        let reference = slide.reference.as_ref()?;
        let child = match size {
            Some(size) => host.child_sized(ui, size, reference.id, reference.block_type),
            None => host.child(ui, reference.id, reference.block_type),
        };
        let ratio = child.aspect_ratio().or_else(|| {
            child
                .intrinsic_size()
                .filter(|size| size.x > 0.0 && size.y > 0.0)
                .map(|size| size.x / size.y)
        });
        if let Some(ratio) = ratio {
            self.shapes.insert(reference.id, ratio);
        }
        self.sidebars.insert(
            reference.id,
            (child.has_left_sidebar(), child.has_right_sidebar()),
        );
        Some(child)
    }

    fn selected_reference(&mut self) -> Option<BlockReference> {
        let slides = self.slides()?;
        let index = self.selected_index(&slides)?;
        slides[index].reference.clone()
    }

    fn slide_sidebar(&mut self, ui: &mut egui::Ui, slide: &Slide) -> egui::Rect {
        let available = ui.available_rect_before_wrap();
        let (Some(host), Some(reference)) = (self.host(), slide.reference.as_ref()) else {
            return available;
        };
        if !self
            .sidebars
            .get(&reference.id)
            .is_some_and(|(left, _)| *left)
        {
            return available;
        }
        let width = SLIDE_SIDEBAR_WIDTH.min(available.width() * 0.5);
        let (sidebar, rest) = (
            egui::Rect::from_min_size(available.min, egui::vec2(width, available.height())),
            available.with_min_x(available.left() + width + 1.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(sidebar), |ui| {
            host.child_part_sized(
                ui,
                ChildPart::LeftSidebar,
                sidebar.size(),
                reference.id,
                reference.block_type,
            );
        });
        ui.painter().vline(
            rest.left() - 0.5,
            available.y_range(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );
        rest
    }

    fn slide_ratio(&self, slide: &Slide) -> f32 {
        slide
            .block_id
            .and_then(|id| self.shapes.get(&id).copied())
            .unwrap_or(DEFAULT_SLIDE_SIZE.x / DEFAULT_SLIDE_SIZE.y)
    }

    fn missing_slide(ui: &mut egui::Ui, slide: &Slide) {
        ui.centered_and_justified(|ui| {
            ui.weak(match slide.block_id {
                Some(_) => "Loading this slide…",
                None => "This slide's block could not be found.",
            });
        });
    }

    fn empty_ui(&mut self, ui: &mut egui::Ui, editable: bool) {
        let mut add = false;
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Empty presentation");
                ui.weak("Add a slide to start this deck.");
                add = editable
                    && ui
                        .button(format!("{} Add slide", ICON_ADD.codepoint))
                        .clicked();
            });
        });
        if add {
            self.open_picker(0);
        }
    }

    fn editing_ui(&mut self, ui: &mut egui::Ui, slides: &[Slide], editable: bool) -> bool {
        let Some(index) = self.selected_index(slides) else {
            self.empty_ui(ui, editable);
            return false;
        };
        let slide = &slides[index];
        let stage = self.slide_sidebar(ui, slide);
        let child = ui
            .scope_builder(egui::UiBuilder::new().max_rect(stage), |ui| {
                self.place_slide(ui, slide, Some(stage.size()))
            })
            .inner;
        let Some(child) = child else {
            Self::missing_slide(ui, slide);
            return false;
        };
        match child.error() {
            Some(error) => {
                ui.painter().text(
                    child.rect().center(),
                    egui::Align2::CENTER_CENTER,
                    error,
                    egui::FontId::proportional(14.0),
                    ui.visuals().error_fg_color,
                );
            }
            None => child.keep_active(),
        }
        child.has_right_sidebar()
    }

    fn playback_keys(&mut self, ui: &egui::Ui, slides: &[Slide]) {
        let current = self.selected_index(slides).unwrap_or(0);
        let (previous, next, first, last) = ui.ctx().input_mut(|input| {
            (
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::PageUp),
                input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::Space)
                    || input.consume_key(egui::Modifiers::NONE, egui::Key::PageDown),
                input.consume_key(egui::Modifiers::NONE, egui::Key::Home),
                input.consume_key(egui::Modifiers::NONE, egui::Key::End),
            )
        });
        if previous {
            self.select(slides, current.saturating_sub(1));
        }
        if next {
            self.select(slides, current + 1);
        }
        if first {
            self.select(slides, 0);
        }
        if last {
            self.select(slides, slides.len().saturating_sub(1));
        }
    }

    fn playback_ui(&mut self, ui: &mut egui::Ui, slides: &[Slide]) {
        let rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(rect, 0.0, egui::Color32::BLACK);
        let Some(index) = self.selected_index(slides) else {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "This presentation has no slides",
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
            return;
        };
        let slide = &slides[index];
        let stage = fit_rect(rect, self.slide_ratio(slide));
        let child = ui
            .scope_builder(egui::UiBuilder::new().max_rect(stage), |ui| {
                self.place_slide(ui, slide, Some(stage.size()))
            })
            .inner;
        match child {
            Some(child) => child.set_mode(ChildMode::Preview),
            None => Self::missing_slide(ui, slide),
        }
        if let Some(step) = self.playback_controls(ui, rect, index, slides.len()) {
            self.select(slides, index.saturating_add_signed(step));
        }
    }

    fn playback_controls(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        index: usize,
        count: usize,
    ) -> Option<isize> {
        let controls_height = PLAYBACK_CONTROLS_HEIGHT.min(rect.height() * 0.3);
        let controls = egui::Rect::from_min_max(
            egui::pos2(rect.left(), rect.bottom() - controls_height),
            rect.max,
        );
        let (now, pointer, moved) = ui.ctx().input(|input| {
            (
                input.time,
                input.pointer.hover_pos(),
                input.pointer.delta() != egui::Vec2::ZERO,
            )
        });
        let over_controls = pointer.is_some_and(|pointer| controls.contains(pointer));
        if over_controls || (moved && pointer.is_some_and(|pointer| rect.contains(pointer))) {
            self.controls_visible_until = now + CONTROLS_VISIBLE_SECONDS;
        }
        if now > self.controls_visible_until {
            return None;
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
        let mut step = None;
        let mut exit = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(controls), |ui| {
            egui::Frame::new()
                .fill(egui::Color32::from_black_alpha(180))
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
                                .add_enabled(index > 0, egui::Button::new(ICON_ARROW_BACK))
                                .on_hover_text("Previous slide")
                                .clicked()
                            {
                                step = Some(-1);
                            }
                            ui.colored_label(
                                egui::Color32::WHITE,
                                format!("{} / {count}", index + 1),
                            );
                            if ui
                                .add_enabled(
                                    index + 1 < count,
                                    egui::Button::new(ICON_ARROW_FORWARD),
                                )
                                .on_hover_text("Next slide")
                                .clicked()
                            {
                                step = Some(1);
                            }
                            exit = ui
                                .button(ICON_CLOSE)
                                .on_hover_text("Stop presenting")
                                .clicked();
                        },
                    );
                });
        });
        if exit {
            if let Some(host) = self.host() {
                host.present(false);
            }
        }
        step
    }

    fn filmstrip_tile(
        &mut self,
        ui: &mut egui::Ui,
        types: &dyn BlockTypes,
        slide: &Slide,
        index: usize,
        editable: bool,
    ) -> (egui::Response, bool) {
        let selected = self.selected == Some(slide.id);
        let mut remove = false;
        let response = egui::Frame::new()
            .fill(match selected {
                true => ui.visuals().selection.bg_fill,
                false => ui.visuals().faint_bg_color,
            })
            .stroke(egui::Stroke::new(
                if selected { 2.0_f32 } else { 1.0_f32 },
                match selected {
                    true => ui.visuals().selection.stroke.color,
                    false => ui.visuals().widgets.noninteractive.bg_stroke.color,
                },
            ))
            .corner_radius(6.0)
            .inner_margin(6.0)
            .show(ui, |ui| {
                let response = match self.place_slide(ui, slide, Some(THUMBNAIL_SIZE)) {
                    Some(child) => {
                        child.set_mode(ChildMode::Preview);
                        child.response.clone()
                    }
                    None => {
                        let (rect, response) =
                            ui.allocate_exact_size(THUMBNAIL_SIZE, egui::Sense::click_and_drag());
                        ui.painter().rect_filled(
                            rect,
                            4.0,
                            ui.visuals().widgets.noninteractive.bg_fill,
                        );
                        response
                    }
                };
                ui.horizontal(|ui| {
                    ui.small(format!("{}", index + 1));
                    let name = slide.reference.as_ref().map_or_else(
                        || {
                            egui::RichText::new(match slide.block_id {
                                Some(_) => "Loading…",
                                None => "Broken link",
                            })
                        },
                        |reference| BlockLabel::for_reference(types, reference).rich_text(),
                    );
                    ui.add(
                        egui::Label::new(name)
                            .truncate()
                            .sense(egui::Sense::click_and_drag()),
                    );
                    if editable
                        && ui
                            .small_button(ICON_DELETE)
                            .on_hover_text("Detach slide")
                            .clicked()
                    {
                        remove = true;
                    }
                });
                response
            })
            .inner;
        (response, remove)
    }
}

impl block_editor_plugin::App for PresentationApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        host.show_region(EditorRegion::RightSidebar, false);
        let block = client.get_block(block_id);
        let dependencies = client.watch_references(BlockReferenceList::References(block_id));
        self.editor = Some(Editor {
            host,
            client,
            block,
            dependencies,
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
        Ok(client.create_block(Presentation::new()).id())
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let Some(slides) = self.slides() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        self.poll_picker(slides.len());
        let Some(host) = self.host() else {
            return;
        };
        let right_sidebar = if host.presenting() {
            self.playback_keys(ui, &slides);
            self.playback_ui(ui, &slides);
            false
        } else if slides.is_empty() {
            self.empty_ui(ui, host.editable());
            false
        } else {
            self.editing_ui(ui, &slides, host.editable())
        };
        host.show_region(EditorRegion::RightSidebar, right_sidebar);
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let Some(slides) = self.slides() else {
            return;
        };
        let rect = ui.available_rect_before_wrap();
        let Some(slide) = slides.first() else {
            ui.centered_and_justified(|ui| {
                ui.weak("Empty presentation");
            });
            return;
        };
        let stage = fit_rect(rect, self.slide_ratio(slide));
        let child = ui
            .scope_builder(egui::UiBuilder::new().max_rect(stage), |ui| {
                self.place_slide(ui, slide, Some(stage.size()))
            })
            .inner;
        match child {
            Some(child) => child.set_mode(ChildMode::Preview),
            None => Self::missing_slide(ui, slide),
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let selected = self.selected?;
        let editor = self.editor.as_ref()?;
        let presentation = editor.block.read()?;
        let slide = presentation.slides().iter().find(|it| it.id == selected)?;
        let block_id = self.reference_cache.peek(&slide.block_id)?;
        drop(presentation);
        let ratio = self.shapes.get(&block_id).copied()?;
        Some(egui::vec2(
            DEFAULT_SLIDE_SIZE.y * ratio,
            DEFAULT_SLIDE_SIZE.y,
        ))
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(host) = self.host() else {
            return;
        };
        let count = self
            .editor
            .as_ref()
            .and_then(|editor| editor.block.read())
            .map_or(0, |presentation| presentation.slides().len());
        let mut add = false;
        let mut present = false;
        let slide = self.selected_reference();
        ui.horizontal(|ui| {
            add = ui
                .add_enabled(
                    host.editable(),
                    egui::Button::new(format!("{} Add slide", ICON_ADD.codepoint)),
                )
                .on_hover_text("Add a slide from a template, a new block or an existing one")
                .clicked();
            present = ui
                .add_enabled(
                    count > 0 && !host.presenting(),
                    egui::Button::new(ICON_FULLSCREEN),
                )
                .on_hover_text("Present")
                .clicked();
            let Some(reference) = slide else {
                return;
            };
            ui.separator();
            let height = self.slide_toolbar.max(ui.spacing().interact_size.y);
            let size = egui::vec2(ui.available_width(), height);
            let child = host.child_part_sized(
                ui,
                ChildPart::Toolbar,
                size,
                reference.id,
                reference.block_type,
            );
            if let Some(used) = child.intrinsic_size() {
                self.slide_toolbar = used.y;
            }
        });
        if add {
            self.open_picker(count);
        }
        if present {
            host.present(true);
        }
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let (Some(host), Some(reference)) = (self.host(), self.selected_reference()) else {
            return;
        };
        host.child_part_sized(
            ui,
            ChildPart::RightSidebar,
            ui.available_size_before_wrap(),
            reference.id,
            reference.block_type,
        );
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.set_width(FILMSTRIP_WIDTH);
        self.poll();
        let Some(slides) = self.slides() else {
            ui.spinner();
            return;
        };
        let Some(host) = self.host() else {
            return;
        };
        if slides.is_empty() {
            ui.weak("Add a slide to start this deck.");
            return;
        }
        let editable = host.editable();
        let types = host.block_types();
        let mut reorder = None;
        let mut removed = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, slide) in slides.iter().enumerate() {
                let (response, remove) =
                    self.filmstrip_tile(ui, types.as_ref(), slide, index, editable);
                if remove {
                    removed = Some(slide.id);
                }
                if response.clicked() {
                    self.selected = Some(slide.id);
                }
                if editable && response.drag_started() {
                    self.dragging = Some(slide.id);
                }
                if response.hovered()
                    && ui.input(|input| input.pointer.any_released())
                    && self.dragging.is_some_and(|dragging| dragging != slide.id)
                {
                    reorder = self.dragging.take().map(|dragging| (dragging, index));
                }
                ui.add_space(8.0);
            }
        });
        if let Some((slide_id, index)) = reorder {
            self.operate(PresentationOperation::Move { slide_id, index });
        }
        if let Some(slide_id) = removed {
            self.remove_slide(&slides, slide_id);
        }
        if ui.input(|input| input.pointer.any_released()) {
            self.dragging = None;
        }
    }
}

fn fit_rect(available: egui::Rect, ratio: f32) -> egui::Rect {
    let ratio = ratio.max(f32::EPSILON);
    let width = available.width().min(available.height() * ratio);
    let height = width / ratio;
    egui::Rect::from_center_size(available.center(), egui::vec2(width, height))
}
