mod font;
mod profiling;
#[cfg(test)]
mod tests;
use std::{
    collections::HashMap,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};

use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    block_url, blocks::text::TextDocument, parse_block_urls, BlockClient, BlockHandle,
    BlockRelationships, ReferenceList,
};
use eframe::egui::{
    self, Color32, Event, EventFilter, ImeEvent, Key, Modifiers, PointerButton, Pos2, Rect, Sense,
    Vec2,
};
use egui_material_icons::icons::{
    ICON_CHECK, ICON_CHECKLIST, ICON_CODE, ICON_DESCRIPTION, ICON_FORMAT_BOLD, ICON_FORMAT_ITALIC,
    ICON_FORMAT_LIST_BULLETED, ICON_FORMAT_LIST_NUMBERED, ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE,
    ICON_LINK, ICON_TITLE,
};
use text_editor_core::{
    CopyMode, Core, CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode,
    EditorCommand, LRDirection, Language, MarkdownCommand, MoveMode, SynHlColorScope,
    SyntaxHighlight, SyntaxNodeDirection, UDDirection, VerticalMoveMode,
};
use uuid::Uuid;

use crate::block_picker::{BlockPicker, BlockPickerMenuAction};

use self::font::{BytePosition, DocumentLayout, ResolvedEmbed, TextRenderer};
use self::profiling::{FrameProfile, PaintTimings, TextProfiler};
use super::{
    clipboard::{ClipboardImagePaste, ClipboardImagePasteResult},
    image::{create_image_block, pick_image_file},
    BlockEditor, BlockRenderContext, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorRegistration, SidebarDragPayload,
};

const PADDING: Vec2 = Vec2::new(12.0, 8.0);
const DIRECT_EDITOR_WIDTH: f32 = 600.0;
const MULTI_CLICK_DELAY: f64 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedEmbed {
    range: Range<usize>,
    id: Uuid,
    large: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MarkdownCheckbox {
    line_start: usize,
    marker: Range<usize>,
    checked: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HighlightLanguage {
    Markdown,
    PlainText,
    Zig,
}

impl HighlightLanguage {
    const fn label(self) -> &'static str {
        match self {
            Self::Markdown => "Markdown",
            Self::PlainText => "Plain text",
            Self::Zig => "Zig",
        }
    }

    const fn core_language(self) -> Option<Language> {
        match self {
            Self::Markdown => Some(Language::Markdown),
            Self::PlainText => None,
            Self::Zig => Some(Language::Zig),
        }
    }
}

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: TextDocument::TYPE_ID,
        display_name: "Text",
        icon: ICON_DESCRIPTION,
        create: Some(|client| {
            let block = client.create_block(TextDocument::new());
            Box::new(TextEditor::new(block, client))
        }),
        open: |client: &BlockClient, id| {
            Box::new(TextEditor::new(
                client.get_block::<TextDocument>(id),
                client,
            ))
        },
        can_add_child: false,
        can_delete_child: false,
        regenerate_dynamic_artifact: None,
    }
}

struct TextEditor {
    block: BlockHandle<TextDocument>,
    core: Core,
    renderer: Result<TextRenderer, String>,
    selecting: bool,
    highlight_language: HighlightLanguage,
    click_count: u8,
    last_click: Option<(f64, Pos2)>,
    profiler: TextProfiler,
    toolbar_profile: Duration,
    layout_cache: Option<CachedLayout>,
    picker: BlockPicker,
    dependencies: ReferenceList,
    pending_create: bool,
    clipboard_image_paste: ClipboardImagePaste,
    image_import_error: Option<String>,
}

struct CachedLayout {
    bytes: Vec<u8>,
    language: HighlightLanguage,
    embeds: Vec<ResolvedEmbed>,
    width: f32,
    layout: Arc<DocumentLayout>,
}

impl TextEditor {
    fn new(block: BlockHandle<TextDocument>, client: &BlockClient) -> Self {
        let mut core = Core::new(block.clone());
        core.set_syntax_highlighter(Some(Language::Markdown));
        core.execute_command(EditorCommand::SetCursorPosition(core.position(0)));
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            core,
            renderer: TextRenderer::new(),
            selecting: false,
            highlight_language: HighlightLanguage::Markdown,
            click_count: 0,
            last_click: None,
            profiler: TextProfiler::default(),
            toolbar_profile: Duration::default(),
            layout_cache: None,
            picker: BlockPicker::default(),
            dependencies,
            pending_create: false,
            clipboard_image_paste: ClipboardImagePaste::default(),
            image_import_error: None,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, editors: &mut EditorAccess<'_>) -> Option<Uuid> {
        let previous = self.highlight_language;
        let mut create_block = None;
        ui.horizontal(|ui| {
            ui.label("Language:");
            egui::ComboBox::from_id_salt(("text-editor-language", self.block.id()))
                .selected_text(self.highlight_language.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.highlight_language,
                        HighlightLanguage::Markdown,
                        HighlightLanguage::Markdown.label(),
                    );
                    ui.selectable_value(
                        &mut self.highlight_language,
                        HighlightLanguage::PlainText,
                        HighlightLanguage::PlainText.label(),
                    );
                    ui.selectable_value(
                        &mut self.highlight_language,
                        HighlightLanguage::Zig,
                        HighlightLanguage::Zig.label(),
                    );
                });
            self.profiler.toggle(ui);
            if self.highlight_language == HighlightLanguage::Markdown {
                ui.separator();
                if ui.button(ICON_FORMAT_BOLD).on_hover_text("Bold").clicked() {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Bold));
                }
                if ui
                    .button(ICON_FORMAT_ITALIC)
                    .on_hover_text("Italic")
                    .clicked()
                {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Italic));
                }
                if ui
                    .button(ICON_FORMAT_STRIKETHROUGH)
                    .on_hover_text("Strikethrough")
                    .clicked()
                {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Strikethrough));
                }
                if ui.button(ICON_CODE).on_hover_text("Inline code").clicked() {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::InlineCode));
                }
                ui.menu_button(ICON_TITLE, |ui| {
                    for level in 1..=6 {
                        if ui.button(format!("Heading {level}")).clicked() {
                            self.core.execute_command(EditorCommand::Markdown(
                                MarkdownCommand::Heading(level),
                            ));
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("Heading");
                if ui
                    .button(ICON_FORMAT_LIST_BULLETED)
                    .on_hover_text("Bulleted list")
                    .clicked()
                {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::BulletedList));
                }
                if ui
                    .button(ICON_FORMAT_LIST_NUMBERED)
                    .on_hover_text("Numbered list")
                    .clicked()
                {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::NumberedList));
                }
                if ui
                    .button(ICON_CHECKLIST)
                    .on_hover_text("Checklist")
                    .clicked()
                {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Checklist));
                }
                if ui.button(ICON_LINK).on_hover_text("Link").clicked() {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Link));
                }
                if ui.button(ICON_IMAGE).on_hover_text("Image").clicked() {
                    self.core
                        .execute_command(EditorCommand::Markdown(MarkdownCommand::Image));
                }
            }
            ui.menu_button("Insert", |ui| {
                ui.menu_button("Block", |ui| {
                    if let Some(action) = BlockPicker::show_menu(ui, editors.registry()) {
                        match action {
                            BlockPickerMenuAction::New(block_type) => {
                                self.pending_create = true;
                                create_block = Some(block_type);
                            }
                            BlockPickerMenuAction::ImportImage => match pick_image_file() {
                                Ok(Some(image)) => {
                                    let source_name = image.source_name().to_owned();
                                    let id = create_image_block(editors, image, self.block.id());
                                    self.insert_image_embed(id, &source_name);
                                    self.image_import_error = None;
                                }
                                Ok(None) => {}
                                Err(error) => self.image_import_error = Some(error),
                            },
                            BlockPickerMenuAction::LinkExisting => {
                                self.picker.open([self.block.id()]);
                            }
                        }
                    }
                });
            });
        });
        if self.highlight_language != previous {
            self.core
                .set_syntax_highlighter(self.highlight_language.core_language());
        }
        create_block
    }

    fn insert_embed(&mut self, id: Uuid) {
        let directive = block_url(id);
        self.core
            .execute_command(EditorCommand::InsertText(directive.as_bytes()));
    }

    fn insert_image_embed(&mut self, id: Uuid, source_name: &str) {
        let directive = image_embed_directive(
            id,
            source_name,
            self.highlight_language == HighlightLanguage::Markdown,
        );
        self.core
            .execute_command(EditorCommand::InsertText(directive.as_bytes()));
    }

    fn paste_clipboard_image(
        &mut self,
        ui: &egui::Ui,
        id: egui::Id,
        editors: &mut EditorAccess<'_>,
    ) -> bool {
        let focused = ui.memory(|memory| memory.has_focus(id));
        let Some(result) = self.clipboard_image_paste.poll(ui.ctx(), focused) else {
            return false;
        };
        match result {
            ClipboardImagePasteResult::NoImage => false,
            ClipboardImagePasteResult::Error(error) => {
                self.image_import_error = Some(error);
                false
            }
            ClipboardImagePasteResult::Image(image) => {
                let source_name = image.source_name().to_owned();
                let image_id = create_image_block(editors, image, self.block.id());
                self.insert_image_embed(image_id, &source_name);
                self.image_import_error = None;
                true
            }
        }
    }

    fn show_picker(&mut self, context: &egui::Context, client: &BlockClient) -> Option<Uuid> {
        let block = self.picker.show(context, client)?;
        self.insert_embed(block.id);
        Some(block.id)
    }

    fn resolve_embeds(&self, bytes: &[u8], editors: &mut EditorAccess<'_>) -> Vec<ResolvedEmbed> {
        let parsed = parse_embeds(
            bytes,
            self.highlight_language == HighlightLanguage::Markdown,
        );
        let references = self.dependencies.read();
        let referenced = references
            .iter()
            .map(|reference| (reference.id, (reference.block_type, reference.name.clone())))
            .collect::<HashMap<_, _>>();
        parsed
            .into_iter()
            .filter(|embed| embed.id != self.block.id())
            .map(|embed| {
                let metadata = referenced.get(&embed.id).cloned().or_else(|| {
                    editors
                        .client()
                        .cached_block(embed.id)
                        .map(|block| (block.block_type, block.name))
                });
                if embed.large {
                    if let Some((block_type, _)) = &metadata {
                        editors.ensure(embed.id, *block_type);
                    }
                }
                let preview_aspect_ratio = embed
                    .large
                    .then(|| editors.render_aspect_ratio(embed.id))
                    .flatten();
                let label = metadata
                    .as_ref()
                    .map(|(_, name)| name)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| embed.id.to_string());
                ResolvedEmbed {
                    range: embed.range,
                    id: embed.id,
                    label,
                    icon: metadata
                        .as_ref()
                        .and_then(|(block_type, _)| editors.registry().icon(*block_type))
                        .map(|icon| icon.codepoint),
                    large: embed.large,
                    available: metadata.is_some(),
                    preview_aspect_ratio,
                }
            })
            .collect()
    }

    fn keyboard_input(&mut self, ui: &egui::Ui, id: egui::Id, suppress_text_paste: bool) -> bool {
        if !ui.memory(|memory| memory.has_focus(id)) {
            return false;
        }
        let event_filter = EventFilter {
            tab: true,
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: false,
        };
        ui.memory_mut(|memory| memory.set_focus_lock_filter(id, event_filter));
        let events = ui.input(|input| input.filtered_events(&event_filter));
        let mut reveal_cursor = false;
        for event in events {
            reveal_cursor |= match event {
                Event::Copy => {
                    let text = self.core.copy_utf8(CopyMode::Copy);
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                    false
                }
                Event::Cut => {
                    let text = self.core.copy_utf8(CopyMode::Cut);
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                    true
                }
                Event::Paste(_) if suppress_text_paste => false,
                Event::Paste(text) => {
                    self.core
                        .execute_command(EditorCommand::Paste(text.as_bytes()));
                    true
                }
                Event::Text(text) if text != "\n" && text != "\r" => {
                    self.core
                        .execute_command(EditorCommand::InsertText(text.as_bytes()));
                    true
                }
                Event::Ime(ImeEvent::Commit(text)) if !text.is_empty() => {
                    self.core
                        .execute_command(EditorCommand::InsertText(text.as_bytes()));
                    true
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => self.key(key, modifiers),
                _ => false,
            };
        }
        reveal_cursor
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let direction = match key {
            Key::ArrowLeft | Key::Home => Some(LRDirection::Left),
            Key::ArrowRight | Key::End => Some(LRDirection::Right),
            _ => None,
        };
        if let Some(direction) = direction {
            self.core
                .execute_command(EditorCommand::MoveCursorLeftRight {
                    mode: if modifiers.shift {
                        MoveMode::Select
                    } else {
                        MoveMode::Move
                    },
                    direction,
                    stop: if matches!(key, Key::Home | Key::End) {
                        CursorLeftRightStop::Line
                    } else if modifiers.alt || modifiers.command {
                        CursorLeftRightStop::Word
                    } else {
                        CursorLeftRightStop::UnicodeGraphemeCluster
                    },
                });
            return true;
        }

        if matches!(key, Key::ArrowUp | Key::ArrowDown) {
            let direction = if key == Key::ArrowUp {
                UDDirection::Up
            } else {
                UDDirection::Down
            };
            if modifiers.alt && modifiers.command {
                self.core.execute_command(EditorCommand::MoveCursorUpDown {
                    direction,
                    mode: VerticalMoveMode::Duplicate,
                    metric: CursorHorizontalPositionMetric::Byte,
                    stop: CursorLeftRightStop::UnicodeGraphemeCluster,
                });
            } else if modifiers.alt {
                self.core.execute_command(EditorCommand::SelectSyntaxNode(
                    if direction == UDDirection::Up {
                        SyntaxNodeDirection::Parent
                    } else {
                        SyntaxNodeDirection::Child
                    },
                ));
            } else {
                self.core.execute_command(EditorCommand::MoveCursorUpDown {
                    direction,
                    mode: if modifiers.shift {
                        VerticalMoveMode::Select
                    } else {
                        VerticalMoveMode::Move
                    },
                    metric: CursorHorizontalPositionMetric::Byte,
                    stop: CursorLeftRightStop::UnicodeGraphemeCluster,
                });
            }
            return true;
        }

        match key {
            Key::Backspace | Key::Delete => {
                self.core.execute_command(EditorCommand::Delete {
                    direction: if key == Key::Backspace {
                        LRDirection::Left
                    } else {
                        LRDirection::Right
                    },
                    stop: if modifiers.alt || modifiers.command {
                        CursorLeftRightStop::Word
                    } else {
                        CursorLeftRightStop::UnicodeGraphemeCluster
                    },
                });
            }
            Key::Enter if modifiers.command => {
                self.core
                    .execute_command(EditorCommand::InsertLine(if modifiers.shift {
                        UDDirection::Up
                    } else {
                        UDDirection::Down
                    }));
            }
            Key::Enter => self.core.execute_command(EditorCommand::Newline),
            Key::Tab => {
                self.core
                    .execute_command(EditorCommand::IndentSelection(if modifiers.shift {
                        LRDirection::Left
                    } else {
                        LRDirection::Right
                    }))
            }
            Key::A if modifiers.command => self.core.execute_command(EditorCommand::SelectAll),
            Key::B if modifiers.command => self
                .core
                .execute_command(EditorCommand::Markdown(MarkdownCommand::Bold)),
            Key::I if modifiers.command => self
                .core
                .execute_command(EditorCommand::Markdown(MarkdownCommand::Italic)),
            Key::Z if modifiers.command => {
                self.core.execute_command(if modifiers.shift {
                    EditorCommand::Redo
                } else {
                    EditorCommand::Undo
                });
            }
            Key::Y if modifiers.command => self.core.execute_command(EditorCommand::Redo),
            Key::D if modifiers.command && modifiers.shift => self
                .core
                .execute_command(EditorCommand::DuplicateLine(UDDirection::Down)),
            Key::D if modifiers.command => self
                .core
                .execute_command(EditorCommand::DuplicateCursor(LRDirection::Right)),
            _ => return false,
        }
        true
    }

    fn pointer_input(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        origin: Pos2,
        layout: &DocumentLayout,
        checkboxes: &[MarkdownCheckbox],
    ) -> bool {
        let (pressed, down, time, modifiers) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.time,
                input.modifiers,
            )
        });
        let Some(pointer) = response.interact_pointer_pos() else {
            if !down {
                self.selecting = false;
            }
            return false;
        };
        let local_pointer = pointer - origin;
        let target = hit_test(layout, local_pointer);
        if pressed && response.contains_pointer() {
            response.request_focus();
            if let Some(checkbox) = checkboxes.iter().find(|checkbox| {
                checkbox_rect(layout, checkbox)
                    .is_some_and(|rect| rect.contains(local_pointer.to_pos2()))
            }) {
                self.core.execute_command(EditorCommand::Markdown(
                    MarkdownCommand::ToggleCheckbox(self.core.position(checkbox.line_start)),
                ));
                self.selecting = false;
                return true;
            }
            if let Some(embed) = layout
                .embeds
                .iter()
                .find(|embed| embed.rect.contains(local_pointer.to_pos2()))
            {
                self.core.execute_command(EditorCommand::Click {
                    position: self.core.position(embed.range.start),
                    mode: DragSelectionMode::move_to(CursorLeftRightStop::Byte),
                    extend: false,
                    select_syntax_node: false,
                });
                self.core
                    .execute_command(EditorCommand::Drag(self.core.position(embed.range.end)));
                self.selecting = false;
                self.click_count = 0;
                self.last_click = None;
                return true;
            }
            self.click_count = self.last_click.map_or(1, |(last_time, last_position)| {
                if time - last_time <= MULTI_CLICK_DELAY
                    && pointer.distance(last_position) <= MULTI_CLICK_DISTANCE
                {
                    self.click_count.saturating_add(1).min(4)
                } else {
                    1
                }
            });
            self.last_click = Some((time, pointer));
            if self.click_count == 4 {
                self.core.execute_command(EditorCommand::SelectAll);
                self.selecting = false;
                self.click_count = 0;
                self.last_click = None;
                return true;
            }
            let mode = match self.click_count {
                2 => DragSelectionMode::select(CursorLeftRightStop::Word),
                3 => DragSelectionMode::select(CursorLeftRightStop::Line),
                _ => DragSelectionMode::default(),
            };
            self.core.execute_command(EditorCommand::Click {
                position: self.core.position(target),
                mode,
                extend: modifiers.shift,
                select_syntax_node: modifiers.alt != modifiers.ctrl,
            });
            self.selecting = true;
            return true;
        }
        if self.selecting && down {
            self.core
                .execute_command(EditorCommand::Drag(self.core.position(target)));
            return true;
        }
        if !down {
            self.selecting = false;
        }
        false
    }

    fn paint(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
        highlight: &SyntaxHighlight,
    ) -> (Option<Rect>, PaintTimings) {
        let selection_start = Instant::now();
        let selection_color = ui.visuals().selection.bg_fill;
        let cursor_color = ui.visuals().selection.stroke.color;
        let mut cursor_rect = None;
        let mut selected_bytes = vec![false; layout.positions.len().saturating_sub(1)];
        for cursor in self.core.cursor_positions() {
            let Some(anchor) = self.core.position_index(cursor.pos.anchor) else {
                continue;
            };
            let Some(focus) = self.core.position_index(cursor.pos.focus) else {
                continue;
            };
            let (start, end) = if anchor <= focus {
                (anchor, focus)
            } else {
                (focus, anchor)
            };
            let selected_len = selected_bytes.len();
            selected_bytes[start.min(selected_len)..end.min(selected_len)].fill(true);
            for byte in start..end {
                if layout.embeds.iter().any(|embed| {
                    !embed.large && byte >= embed.range.start && byte < embed.range.end
                }) {
                    continue;
                }
                let Some(left) = layout.positions.get(byte).and_then(|position| *position) else {
                    continue;
                };
                let right = layout
                    .positions
                    .get(byte + 1)
                    .and_then(|position| *position)
                    .filter(|right| right.line == left.line)
                    .unwrap_or(BytePosition {
                        line: left.line,
                        x: left.x + 8.0,
                    });
                let y = origin.y + layout.lines[left.line].y;
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(origin.x + left.x.min(right.x), y),
                        Pos2::new(
                            origin.x + left.x.max(right.x).max(left.x.min(right.x) + 1.0),
                            y + layout.lines[left.line].height,
                        ),
                    ),
                    0.0,
                    selection_color,
                );
            }
            if let Some(position) = layout.positions.get(focus).and_then(|position| *position) {
                let top = Pos2::new(
                    origin.x + position.x,
                    origin.y + layout.lines[position.line].y,
                );
                let rect =
                    Rect::from_min_size(top, Vec2::new(2.0, layout.lines[position.line].height));
                painter.rect_filled(rect, 0.0, cursor_color);
                cursor_rect = Some(rect);
            }
        }

        let selection = selection_start.elapsed();
        let renderer = match &mut self.renderer {
            Ok(renderer) => renderer,
            Err(_) => {
                return (
                    cursor_rect,
                    PaintTimings {
                        selection,
                        ..PaintTimings::default()
                    },
                )
            }
        };
        let glyph_start = Instant::now();
        let mut rasterize = std::time::Duration::ZERO;
        let mut glyph_count = 0;
        let mut cache_misses = 0;
        for line in &layout.lines {
            let line_profile = renderer.paint_line(
                ui.ctx(),
                painter,
                origin,
                line,
                |byte| syntax_color(highlight.style_at(byte).color),
                |byte| selected_bytes.get(byte).copied().unwrap_or(false),
                syntax_color(SynHlColorScope::Invisible),
            );
            rasterize += line_profile.rasterize;
            glyph_count += line_profile.glyph_count;
            cache_misses += line_profile.cache_misses;
        }
        (
            cursor_rect,
            PaintTimings {
                selection,
                glyphs: glyph_start.elapsed(),
                rasterize,
                glyph_count,
                cache_misses,
            },
        )
    }

    fn paint_embeds(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
        editors: &mut EditorAccess<'_>,
    ) {
        for embed in &layout.embeds {
            let rect = embed.rect.translate(origin.to_vec2());
            if embed.large {
                painter.rect_filled(rect, 6.0, Color32::from_rgb(35, 46, 56));
                let rendered = editors.render(
                    embed.id,
                    BlockRenderContext {
                        painter,
                        corners: [
                            rect.left_top(),
                            rect.right_top(),
                            rect.right_bottom(),
                            rect.left_bottom(),
                        ],
                        opacity: 1.0,
                    },
                );
                if !rendered {
                    let title = painter.layout_no_wrap(
                        embed.label.clone(),
                        egui::FontId::proportional(18.0),
                        ui.visuals().text_color(),
                    );
                    let status = painter.layout_no_wrap(
                        if embed.available {
                            "Preview unavailable".to_owned()
                        } else {
                            "Block unavailable".to_owned()
                        },
                        egui::FontId::proportional(13.0),
                        ui.visuals().weak_text_color(),
                    );
                    let gap = 6.0;
                    let total_height = title.size().y + gap + status.size().y;
                    let top = rect.center().y - total_height * 0.5;
                    painter.galley(
                        Pos2::new(rect.center().x - title.size().x * 0.5, top),
                        title,
                        ui.visuals().text_color(),
                    );
                    painter.galley(
                        Pos2::new(
                            rect.center().x - status.size().x * 0.5,
                            top + total_height - status.size().y,
                        ),
                        status,
                        ui.visuals().weak_text_color(),
                    );
                }
                continue;
            }
            let selected = self.core.cursor_positions().iter().any(|cursor| {
                let Some(anchor) = self.core.position_index(cursor.pos.anchor) else {
                    return false;
                };
                let Some(focus) = self.core.position_index(cursor.pos.focus) else {
                    return false;
                };
                let (start, end) = if anchor <= focus {
                    (anchor, focus)
                } else {
                    (focus, anchor)
                };
                start < embed.range.end && end > embed.range.start
            });
            painter.rect_filled(
                rect,
                5.0,
                if selected {
                    ui.visuals().selection.bg_fill
                } else if embed.available {
                    Color32::from_rgb(49, 65, 78)
                } else {
                    Color32::from_rgb(72, 55, 61)
                },
            );
            if let Some(icon) = embed.icon {
                painter.text(
                    Pos2::new(rect.left() + 13.0, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    icon,
                    egui::FontId::new(
                        16.0,
                        egui::FontFamily::Name(egui_material_icons::FONT_FAMILY.into()),
                    ),
                    ui.visuals().text_color(),
                );
            }
        }
    }

    fn paint_checkboxes(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
        checkboxes: &[MarkdownCheckbox],
    ) {
        for checkbox in checkboxes {
            let Some(marker_rect) = checkbox_marker_rect(layout, checkbox) else {
                continue;
            };
            let Some(rect) = checkbox_rect(layout, checkbox) else {
                continue;
            };
            let marker_rect = marker_rect.translate(origin.to_vec2());
            let rect = rect.translate(origin.to_vec2());
            painter.rect_filled(marker_rect, 0.0, Color32::from_rgb(29, 37, 44));
            painter.rect(
                rect,
                3.0,
                if checkbox.checked {
                    ui.visuals().selection.bg_fill
                } else {
                    Color32::TRANSPARENT
                },
                egui::Stroke::new(1.5, ui.visuals().widgets.inactive.fg_stroke.color),
                egui::StrokeKind::Inside,
            );
            if checkbox.checked {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    ICON_CHECK.codepoint,
                    egui::FontId::new(14.0, ICON_CHECK.font_family()),
                    ui.visuals().selection.stroke.color,
                );
            }
        }
    }

    fn selected_inline_embed<'a>(
        &self,
        layout: &'a DocumentLayout,
    ) -> Option<&'a font::EmbedLayout> {
        let cursors = self.core.cursor_positions();
        let cursor = (cursors.len() == 1).then(|| &cursors[0])?;
        let anchor = self.core.position_index(cursor.pos.anchor)?;
        let focus = self.core.position_index(cursor.pos.focus)?;
        let selection = anchor.min(focus)..anchor.max(focus);
        layout
            .embeds
            .iter()
            .find(|embed| !embed.large && embed.range == selection)
    }

    fn selected_embed_action(
        &self,
        context: &egui::Context,
        origin: Pos2,
        layout: &DocumentLayout,
        client: &BlockClient,
    ) -> Option<EditorAction> {
        let embed = self.selected_inline_embed(layout)?;
        let rect = embed.rect.translate(origin.to_vec2());
        let cached = client.cached_block(embed.id);
        let mut action = None;
        egui::Area::new(egui::Id::new((
            "open-text-embed",
            self.block.id(),
            embed.range.start,
        )))
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_TOP)
        .fixed_pos(rect.center_bottom() + Vec2::new(0.0, 6.0))
        .show(context, |ui| {
            if ui
                .add_enabled(cached.is_some(), egui::Button::new("Edit"))
                .on_disabled_hover_text("Waiting for cached block metadata")
                .clicked()
            {
                let cached = cached.as_ref().unwrap();
                action = Some(EditorAction::OpenBlock {
                    id: cached.id,
                    block_type: cached.block_type,
                });
            }
        });
        action
    }
}

impl BlockEditor for TextEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        TextDocument::TYPE_ID
    }

    fn name(&self) -> String {
        self.block.name()
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        self.block.read().map(|_| self.block.relationships())
    }

    fn set_parent(&self, parent: BlockParent) {
        self.block.set_parent(parent);
    }

    fn note_backref(&self, id: Uuid) {
        self.block.note_backref(id);
    }

    fn history(&self) -> Option<&dyn block_client::BlockHistoryHandle> {
        Some(&self.block)
    }

    fn block_created(&mut self, id: Uuid, _block_type: Uuid, _author: Uuid, _name: String) -> bool {
        if !std::mem::take(&mut self.pending_create) {
            return false;
        }
        self.insert_embed(id);
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(&mut self, editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        let bytes = self.block.read()?.bytes().to_vec();
        let highlight = self.core.highlight();
        let embeds = self.resolve_embeds(&bytes, editors);
        let height = match &self.renderer {
            Ok(renderer) => {
                renderer
                    .layout_profiled(
                        &bytes,
                        &highlight,
                        &embeds,
                        DIRECT_EDITOR_WIDTH - PADDING.x * 2.0,
                    )
                    .0
                    .size
                    .y
            }
            Err(_) => PADDING.y * 2.0,
        };
        Some(Vec2::new(DIRECT_EDITOR_WIDTH, height))
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.profiler.show(ui.ctx());
        let toolbar_start = Instant::now();
        let create_block = self.toolbar(ui, editors);
        if let Some(error) = self.image_import_error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.image_import_error = None;
                }
            });
        }
        self.toolbar_profile = toolbar_start.elapsed();
        create_block.map(|block_type| EditorAction::CreateBlock {
            block_type,
            parent: Some(self.block.id()),
        })
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let frame_start = Instant::now();
        let mut profile = FrameProfile::default();
        profile.toolbar = std::mem::take(&mut self.toolbar_profile);
        let id = egui::Id::new(("text-editor", self.block.id()));
        let keyboard_start = Instant::now();
        let pasted_image = self.paste_clipboard_image(ui, id, editors);
        let mut reveal_cursor = pasted_image || self.keyboard_input(ui, id, pasted_image);
        profile.keyboard = keyboard_start.elapsed();
        let linked_block = self.show_picker(ui.ctx(), editors.client());
        let document_start = Instant::now();
        let Some(bytes) = self.block.read().map(|document| document.bytes().to_vec()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            profile.document = document_start.elapsed();
            profile.total = frame_start.elapsed() + profile.toolbar;
            self.profiler.record(profile);
            return None;
        };
        profile.document = document_start.elapsed();
        profile.document_bytes = bytes.len();
        let checkboxes = if self.highlight_language == HighlightLanguage::Markdown {
            parse_markdown_checkboxes(&bytes)
        } else {
            Vec::new()
        };
        let highlight_start = Instant::now();
        let highlight = self.core.highlight();
        profile.highlight = highlight_start.elapsed();
        let layout_start = Instant::now();
        let embeds = self.resolve_embeds(&bytes, editors);
        let full_width = (ui.available_width() - PADDING.x * 2.0).max(1.0);
        let layout = if let Some(cached) = self.layout_cache.as_ref().filter(|cached| {
            cached.language == self.highlight_language
                && cached.bytes == bytes
                && cached.embeds == embeds
                && cached.width == full_width
        }) {
            Arc::clone(&cached.layout)
        } else {
            let layout = match &self.renderer {
                Ok(renderer) => {
                    let (layout, detail) =
                        renderer.layout_profiled(&bytes, &highlight, &embeds, full_width);
                    profile.layout_detail = Some(detail);
                    Arc::new(layout)
                }
                Err(error) => {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    });
                    profile.layout = layout_start.elapsed();
                    profile.total = frame_start.elapsed() + profile.toolbar;
                    self.profiler.record(profile);
                    return None;
                }
            };
            self.layout_cache = Some(CachedLayout {
                bytes,
                language: self.highlight_language,
                embeds,
                width: full_width,
                layout: Arc::clone(&layout),
            });
            layout
        };
        profile.layout = layout_start.elapsed();
        profile.line_count = layout.lines.len();

        let desired = layout.size.max(ui.available_size());
        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let response = response.on_hover_cursor(egui::CursorIcon::Text);
        ui.painter()
            .rect_filled(response.rect, 0.0, Color32::from_rgb(29, 37, 44));
        let origin = response.rect.min + PADDING;
        let edit_block = self.selected_embed_action(ui.ctx(), origin, &layout, editors.client());
        let pointer_start = Instant::now();
        reveal_cursor |= self.pointer_input(ui, &response, origin, &layout, &checkboxes);
        profile.pointer = pointer_start.elapsed();
        self.paint_embeds(ui, &painter, origin, &layout, editors);
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.ctx.pointer_hover_pos());
        let drop_index = response
            .dnd_hover_payload::<SidebarDragPayload>()
            .filter(|dragged| dragged.reference.id != self.block.id())
            .and_then(|_| pointer)
            .map(|pointer| hit_test(&layout, pointer - origin))
            .and_then(|byte| {
                self.core.position_index(
                    self.core
                        .cursor_stop(byte, CursorLeftRightStop::UnicodeGraphemeCluster),
                )
            });
        if let Some(byte) = drop_index {
            response.ctx.set_cursor_icon(egui::CursorIcon::Alias);
            if let Some(position) = layout.positions.get(byte).and_then(|position| *position) {
                let top = Pos2::new(
                    origin.x + position.x,
                    origin.y + layout.lines[position.line].y,
                );
                painter.rect_filled(
                    Rect::from_min_size(top, Vec2::new(2.0, layout.lines[position.line].height)),
                    0.0,
                    ui.visuals().selection.stroke.color,
                );
            }
        }
        if let Some(dragged) = response.dnd_release_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                if let Some(byte) = pointer.map(|pointer| hit_test(&layout, pointer - origin)) {
                    let position = self
                        .core
                        .cursor_stop(byte, CursorLeftRightStop::UnicodeGraphemeCluster);
                    self.core
                        .execute_command(EditorCommand::SetCursorPosition(position));
                    self.insert_embed(dragged.reference.id);
                    reveal_cursor = true;
                }
            }
        }
        let (cursor, paint) = self.paint(ui, &painter, origin, &layout, &highlight);
        self.paint_checkboxes(ui, &painter, origin, &layout, &checkboxes);
        profile.paint = paint;
        if reveal_cursor {
            if let Some(cursor) = cursor {
                ui.scroll_to_rect(cursor.expand2(Vec2::new(8.0, 3.0)), None);
            }
        }
        profile.total = frame_start.elapsed() + profile.toolbar;
        self.profiler.record(profile);
        edit_block.or_else(|| {
            linked_block.map(|id| EditorAction::SetParent {
                id,
                parent: self.block.id(),
            })
        })
    }
}

fn image_embed_directive(id: Uuid, source_name: &str, markdown: bool) -> String {
    let url = block_url(id);
    if markdown {
        format!("![{source_name}]({url})")
    } else {
        url
    }
}

fn parse_embeds(bytes: &[u8], markdown: bool) -> Vec<ParsedEmbed> {
    parse_block_urls(bytes)
        .into_iter()
        .map(|url| {
            let image_range = markdown
                .then(|| markdown_image_range(bytes, &url.range))
                .flatten();
            ParsedEmbed {
                range: url.range,
                id: url.id,
                large: image_range.is_some(),
            }
        })
        .collect()
}

fn parse_markdown_checkboxes(bytes: &[u8]) -> Vec<MarkdownCheckbox> {
    let mut result = Vec::new();
    let mut line_start = 0;
    loop {
        let line_end = bytes[line_start..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(bytes.len(), |offset| line_start + offset);
        let indent = bytes[line_start..line_end]
            .iter()
            .take_while(|byte| matches!(byte, b' ' | b'\t'))
            .count();
        let start = line_start + indent;
        if let Some(marker) = bytes.get(start..start + 6) {
            if matches!(marker[0], b'-' | b'*' | b'+')
                && marker[1] == b' '
                && marker[2] == b'['
                && matches!(marker[3], b' ' | b'x' | b'X')
                && marker[4..6] == *b"] "
            {
                result.push(MarkdownCheckbox {
                    line_start,
                    marker: start + 2..start + 5,
                    checked: matches!(marker[3], b'x' | b'X'),
                });
            }
        }
        if line_end == bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }
    result
}

fn checkbox_marker_rect(layout: &DocumentLayout, checkbox: &MarkdownCheckbox) -> Option<Rect> {
    let left = layout
        .positions
        .get(checkbox.marker.start)
        .copied()
        .flatten()?;
    let right = layout
        .positions
        .get(checkbox.marker.end)
        .copied()
        .flatten()?;
    (left.line == right.line).then(|| {
        let line = &layout.lines[left.line];
        Rect::from_min_max(
            Pos2::new(left.x, line.y),
            Pos2::new(right.x.max(left.x + 18.0), line.y + line.height),
        )
    })
}

fn checkbox_rect(layout: &DocumentLayout, checkbox: &MarkdownCheckbox) -> Option<Rect> {
    let marker = checkbox_marker_rect(layout, checkbox)?;
    let size = marker.height().min(18.0);
    Some(Rect::from_center_size(
        Pos2::new(marker.left() + size * 0.5, marker.center().y),
        Vec2::splat(size),
    ))
}

fn markdown_image_range(bytes: &[u8], url: &Range<usize>) -> Option<Range<usize>> {
    if url.start < 3
        || bytes.get(url.start - 2..url.start)? != b"]("
        || bytes.get(url.end) != Some(&b')')
    {
        return None;
    }
    let line_start = bytes[..url.start - 2]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1);
    let image_start = bytes[line_start..url.start - 2]
        .windows(2)
        .rposition(|window| window == b"![")?
        + line_start;
    let image_end = url.end + 1;
    let line_end = bytes[image_end..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |newline| image_end + newline);
    let whitespace = |byte: &u8| matches!(*byte, b' ' | b'\t' | b'\r');
    (bytes[line_start..image_start].iter().all(whitespace)
        && bytes[image_end..line_end].iter().all(whitespace))
    .then_some(image_start..image_end)
}

fn hit_test(layout: &DocumentLayout, point: Vec2) -> usize {
    let line = layout
        .lines
        .iter()
        .position(|line| point.y < line.y + line.height)
        .unwrap_or_else(|| layout.lines.len().saturating_sub(1));
    let line_layout = &layout.lines[line];
    let inline_embeds = layout
        .embeds
        .iter()
        .filter(|embed| !embed.large && embed.rect.center().y >= line_layout.y)
        .filter(|embed| embed.rect.center().y < line_layout.y + line_layout.height)
        .collect::<Vec<_>>();
    (line_layout.start..=line_layout.end)
        .filter(|byte| {
            !inline_embeds
                .iter()
                .any(|embed| *byte > embed.range.start && *byte < embed.range.end)
        })
        .filter_map(|byte| {
            layout.positions[byte]
                .filter(|position| position.line == line)
                .map(|position| (byte, (position.x - point.x).abs()))
        })
        .min_by(|left, right| left.1.total_cmp(&right.1))
        .map_or(line_layout.start, |(byte, _)| byte)
}

fn syntax_color(scope: SynHlColorScope) -> Color32 {
    Color32::from_rgb(
        ((syntax_hex(scope) >> 16) & 0xff) as u8,
        ((syntax_hex(scope) >> 8) & 0xff) as u8,
        (syntax_hex(scope) & 0xff) as u8,
    )
}

fn syntax_hex(scope: SynHlColorScope) -> u32 {
    match scope {
        SynHlColorScope::Invalid => 0xff0000,
        SynHlColorScope::KeywordStorage => 0x008b94,
        SynHlColorScope::Literal => 0xe27e8d,
        SynHlColorScope::VariableFunction => 0x70e1e8,
        SynHlColorScope::PunctuationImportant => 0xb7c5d3,
        SynHlColorScope::Variable => 0x718ca1,
        SynHlColorScope::VariableParameter => 0xebbf83,
        SynHlColorScope::LiteralString => 0x68a1f0,
        SynHlColorScope::KeywordPrimitiveType => 0x70e1e8,
        SynHlColorScope::Punctuation => 0x718ca1,
        SynHlColorScope::Keyword => 0x5ec4ff,
        SynHlColorScope::VariableConstant => 0x8bd49c,
        SynHlColorScope::VariableMutable => 0xb7c5d3,
        SynHlColorScope::Comment => 0xff9d1c,
        SynHlColorScope::MarkdownPlainText => 0xffffff,
        SynHlColorScope::MarkdownSymbol => 0x718ca1,
        SynHlColorScope::MarkdownLink => 0x70e1e8,
        SynHlColorScope::MarkdownCode => 0x8bd49c,
        SynHlColorScope::Unstyled => 0xb7c5d3,
        SynHlColorScope::Invisible => 0x43515c,
    }
}
