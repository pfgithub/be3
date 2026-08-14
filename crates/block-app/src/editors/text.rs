mod font;
#[cfg(test)]
mod tests;
mod timings;
use std::{
    collections::HashMap,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};

use block::{BlockParent, BlockReferenceList, ClientId};
use block_client::{
    block_url,
    blocks::text::{TextDocument, TextIndentation, TextLanguage},
    blocks::workspace_index::BlockEntry,
    parse_block_urls,
    presence::{PresenceColor, UserActive},
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui::{
    self, output::IMEOutput, Color32, CornerRadius, Event, EventFilter, ImeEvent, Key, Modifiers,
    PointerButton, Pos2, Rect, Sense, Vec2,
};
use egui_material_icons::{
    icons::{
        ICON_ARROW_BACK, ICON_CHECK, ICON_CHECKLIST, ICON_CLOSE, ICON_CODE, ICON_DESCRIPTION,
        ICON_FIND_REPLACE, ICON_FORMAT_BOLD, ICON_FORMAT_ITALIC, ICON_FORMAT_LIST_BULLETED,
        ICON_FORMAT_LIST_NUMBERED, ICON_FORMAT_STRIKETHROUGH, ICON_IMAGE, ICON_KEYBOARD_ARROW_DOWN,
        ICON_KEYBOARD_ARROW_RIGHT, ICON_KEYBOARD_ARROW_UP, ICON_LINK, ICON_MATCH_CASE, ICON_SEARCH,
        ICON_TITLE,
    },
    MaterialIcon,
};
use text_editor_core::{
    markdown_checkbox_marker, CollapsibleSection, CopyMode, Core, CursorHorizontalPositionMetric,
    CursorLeftRightStop, CursorPosition, DragSelectionMode, EditorCommand, FindDirection,
    LRDirection, MarkdownCommand, MoveMode, Position, SynHlColorScope, SyntaxHighlight,
    SyntaxNodeDirection, TextCursor, UDDirection, VerticalMoveMode,
};
use uuid::Uuid;

use crate::{block_picker::BlockPicker, performance, presence_color_rgb};

use self::font::{BytePosition, DocumentLayout, LineLayout, ResolvedEmbed, TextRenderer};
use self::timings::{FrameProfile, PaintTimings};
use super::{
    clipboard::{ClipboardImagePaste, ClipboardImagePasteResult},
    embedded_editor_frame_size, embedded_editor_ui,
    image::create_image_block,
    paint_name, BlockEditor, BlockLabel, BlockRenderContext, CreatableEditor,
    DirectEditorCapabilities, DirectEditorInteraction, DirectEditorResize, DirectEditorViewport,
    EditorAccess, EditorAction, EditorKind, SidebarDragPayload, EMBEDDED_EDITOR_PADDING,
    EMBEDDED_EDITOR_TITLE_GAP, EMBEDDED_EDITOR_TITLE_HEIGHT,
};

const PADDING: Vec2 = Vec2::new(12.0, 8.0);
const DIRECT_EDITOR_WIDTH: f32 = 600.0;
const GUTTER_TEXT_SIZE: f32 = 12.0;
const GUTTER_PADDING_LEFT: f32 = 10.0;
const GUTTER_PADDING_RIGHT: f32 = 10.0;
const GUTTER_ARROW_SIZE: f32 = 14.0;
const COLLAPSED_ELLIPSIS_GAP: f32 = 6.0;
const MULTI_CLICK_DELAY: f64 = 0.3;
const MULTI_CLICK_DISTANCE: f32 = 6.0;
const TOUCH_HANDLE_RADIUS: f32 = 9.0;
const TOUCH_HANDLE_GAP: f32 = 4.0;
const TOUCH_HANDLE_HIT_RADIUS: f32 = 24.0;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionHandle {
    Start,
    End,
}

impl EditorKind for TextEditor {
    type Block = TextDocument;

    const DISPLAY_NAME: &'static str = "Text";
    const ICON: MaterialIcon = ICON_DESCRIPTION;
    const CAN_REPLACE_CHILD: bool = true;
    const DEFAULT_ADD: bool = true;

    fn open(client: &BlockClient, block: BlockHandle<TextDocument>) -> Self {
        Self::new(block, client)
    }
}

impl CreatableEditor for TextEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(TextDocument::new()), client)
    }
}

pub(super) struct TextEditor {
    block: BlockHandle<TextDocument>,
    workspace_id: Uuid,
    core: Core,
    renderer: Result<TextRenderer, String>,
    selecting: bool,
    click_count: u8,
    last_click: Option<(f64, Pos2)>,
    touch_mode: bool,
    dragging_handle: Option<Position>,
    dragging_handle_offset: Vec2,
    touch_menu_open: bool,
    touch_menu_pos: Pos2,
    toolbar_profile: Duration,
    layout_cache: Option<CachedLayout>,
    picker: BlockPicker,
    dependencies: ReferenceList,
    clipboard_image_paste: ClipboardImagePaste,
    image_import_error: Option<String>,
    focused_embed: Option<FocusedEmbed>,
    find: Option<FindState>,
    find_reveal: bool,
    last_cursor_positions: Vec<CursorPosition>,
    pending_presence_reveal: Option<ClientId>,
}

/// UI-only state for the find/replace bar: what's typed and which panel is
/// showing. Deliberately independent of [`EditorCommand::DuplicateCursor`]'s
/// "select next occurrence" mechanism: find/replace always operates on a
/// single cursor built from its own query, never on the multi-cursor state
/// ctrl+d builds up.
///
/// The matching, navigation, and replacement logic itself is *not*
/// reimplemented here. Anything that manages document state — locating
/// matches, moving the selection to one, replacing one or all of them — is
/// an [`EditorCommand`] or query on [`text_editor_core::Core`]
/// ([`EditorCommand::Find`], [`EditorCommand::ReplaceMatch`],
/// [`EditorCommand::ReplaceAllMatches`], `find_status`, `find_matches`).
/// This module only renders the bar and dispatches those operations; it
/// should never scan document bytes, track match ranges, or loop over
/// matches applying edits itself.
struct FindState {
    query: String,
    replace: String,
    case_sensitive: bool,
    show_replace: bool,
    focus_query: bool,
}

impl Default for FindState {
    fn default() -> Self {
        Self {
            query: String::new(),
            replace: String::new(),
            case_sensitive: false,
            show_replace: false,
            focus_query: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct FocusedEmbed {
    id: Uuid,
    source_start: usize,
}

struct CachedLayout {
    bytes: Vec<u8>,
    language: TextLanguage,
    embeds: Vec<ResolvedEmbed>,
    hidden: Vec<Range<usize>>,
    wrap_width: f32,
    layout: Arc<DocumentLayout>,
}

impl TextEditor {
    fn new(block: BlockHandle<TextDocument>, client: &BlockClient) -> Self {
        let mut core = Core::new(block.clone());
        let start = core.position(0);
        core.execute_command(EditorCommand::SetSelection {
            anchor: start,
            focus: start,
        });
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            workspace_id: client.workspace_id(),
            core,
            renderer: TextRenderer::new(),
            selecting: false,
            click_count: 0,
            last_click: None,
            touch_mode: false,
            dragging_handle: None,
            dragging_handle_offset: Vec2::ZERO,
            touch_menu_open: false,
            touch_menu_pos: Pos2::ZERO,
            toolbar_profile: Duration::default(),
            layout_cache: None,
            picker: BlockPicker::default(),
            dependencies,
            clipboard_image_paste: ClipboardImagePaste::default(),
            image_import_error: None,
            focused_embed: None,
            find: None,
            find_reveal: false,
            last_cursor_positions: Vec::new(),
            pending_presence_reveal: None,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, _editors: &mut EditorAccess<'_>) {
        let previous = self.core.language();
        let mut language = previous;
        let mut indentation = self.core.indentation();
        ui.horizontal_wrapped(|ui| {
            ui.label("Language:");
            egui::ComboBox::from_id_salt(("text-editor-language", self.block.id()))
                .selected_text(previous.label())
                .show_ui(ui, |ui| {
                    for choice in TextLanguage::ALL {
                        ui.selectable_value(&mut language, choice, choice.label());
                    }
                });
            ui.label("Indentation:");
            egui::ComboBox::from_id_salt(("text-editor-indentation", self.block.id()))
                .selected_text(match indentation {
                    TextIndentation::Tabs => "Tabs",
                    TextIndentation::Spaces { .. } => "Spaces",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut indentation, TextIndentation::Tabs, "Tabs");
                    if ui
                        .selectable_label(
                            matches!(indentation, TextIndentation::Spaces { .. }),
                            "Spaces",
                        )
                        .clicked()
                    {
                        indentation = TextIndentation::Spaces { width: 2 };
                    }
                });
            if let TextIndentation::Spaces { width } = &mut indentation {
                ui.add(egui::DragValue::new(width).range(1..=8).suffix(" spaces"));
            }
            if previous == TextLanguage::Markdown {
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
            if ui.button("Insert").clicked() {
                self.picker.open([self.block.id()]);
            }
            ui.separator();
            if ui
                .button(ICON_FIND_REPLACE)
                .on_hover_text("Find and replace")
                .clicked()
            {
                self.open_find(true);
            }
        });
        if language != previous {
            self.core
                .execute_command(EditorCommand::SetLanguage(language));
        }
        if indentation != self.core.indentation() {
            self.core
                .execute_command(EditorCommand::SetIndentation(indentation));
        }
    }

    fn insert_image_embed(&mut self, id: Uuid, source_name: &str) {
        let directive = image_embed_directive(
            self.workspace_id,
            id,
            source_name,
            self.core.language() == TextLanguage::Markdown,
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

    fn handle_picker(&mut self, context: &egui::Context, editors: &mut EditorAccess<'_>) {
        let Some(result) = self
            .picker
            .handle(context, editors, BlockParent::Uuid(self.block.id()))
        else {
            return;
        };
        if !result.linked {
            editors.set_parent(result.id, BlockParent::Uuid(self.block.id()));
        }
        let name =
            BlockLabel::for_properties(editors.registry(), result.block_type, &result.properties)
                .name;
        self.insert_image_embed(result.id, &name);
    }

    fn resolve_embeds(&self, bytes: &[u8], editors: &mut EditorAccess<'_>) -> Vec<ResolvedEmbed> {
        let parsed = parse_embeds(
            bytes,
            self.workspace_id,
            self.core.language() == TextLanguage::Markdown,
        );
        let references = self.dependencies.read();
        let referenced = references
            .iter()
            .map(|reference| {
                (
                    reference.id,
                    (
                        reference.block_type,
                        block_client::properties::read_name(&reference.properties),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        parsed
            .into_iter()
            .filter(|embed| embed.id != self.block.id())
            .map(|embed| {
                let metadata = referenced.get(&embed.id).cloned().or_else(|| {
                    editors.client().cached_block(embed.id).map(|block| {
                        (
                            block.block_type,
                            block_client::properties::read_name(&block.properties),
                        )
                    })
                });
                if embed.large {
                    if let Some((block_type, _)) = &metadata {
                        editors.ensure(embed.id, *block_type);
                    }
                }
                let frame_size = embed
                    .large
                    .then(|| editors.direct_editor_intrinsic_size(embed.id))
                    .flatten()
                    .map(|intrinsic| embedded_editor_frame_size(intrinsic, 1.0));
                let label = metadata.as_ref().map_or_else(
                    || BlockLabel {
                        block_type: Uuid::nil(),
                        icon: None,
                        name: embed.id.to_string(),
                        automatic: true,
                    },
                    |(block_type, name)| {
                        BlockLabel::new(editors.registry(), *block_type, name.as_ref())
                    },
                );
                ResolvedEmbed {
                    range: embed.range,
                    id: embed.id,
                    label: label.name,
                    icon: label.icon.map(|icon| icon.codepoint),
                    automatic: label.automatic,
                    large: embed.large,
                    available: metadata.is_some(),
                    frame_size,
                }
            })
            .collect()
    }

    /// Whether the primary cursor sits on a currently-collapsed line, to
    /// decide which way [`Key::OpenBracket`] should toggle.
    fn cursor_line_collapsed(&self) -> bool {
        let Some(cursor) = self.core.cursor_positions().first() else {
            return false;
        };
        let line_start = self.core.get_line_start(cursor.pos.focus);
        let Some(line_start) = self.core.position_index(line_start) else {
            return false;
        };
        self.core
            .collapsible_sections()
            .iter()
            .any(|section| section.line_start == line_start && section.collapsed)
    }

    /// Opens the find bar (or the find/replace bar, if `show_replace`),
    /// prefilling the query from the current selection the first time it's
    /// opened.
    fn open_find(&mut self, show_replace: bool) {
        if self.find.is_none() {
            let selected = self.core.copy_utf8(CopyMode::Copy);
            let query = if !selected.is_empty() && !selected.contains('\n') {
                selected
            } else {
                String::new()
            };
            self.find = Some(FindState {
                query,
                ..FindState::default()
            });
        }
        if let Some(find) = self.find.as_mut() {
            find.show_replace = show_replace;
            find.focus_query = true;
        }
        self.find_reveal |= self.sync_find();
    }

    fn close_find(&mut self) {
        self.find = None;
    }

    /// Moves the selection onto a match only if it isn't sitting on one
    /// already, so opening the bar or editing the query doesn't jump past a
    /// match the selection already prefilled it from.
    fn sync_find(&mut self) -> bool {
        let Some(find) = self.find.as_ref() else {
            return false;
        };
        let status = self.core.find_status(&find.query, find.case_sensitive);
        if status.current.is_none() && status.total > 0 {
            self.core.execute_command(EditorCommand::Find {
                text: &find.query,
                case_sensitive: find.case_sensitive,
                direction: FindDirection::Next,
            });
        }
        status.total > 0
    }

    fn find_next(&mut self) -> bool {
        let Some(find) = self.find.as_ref() else {
            return false;
        };
        self.core.execute_command(EditorCommand::Find {
            text: &find.query,
            case_sensitive: find.case_sensitive,
            direction: FindDirection::Next,
        });
        self.core
            .find_status(&find.query, find.case_sensitive)
            .total
            > 0
    }

    fn find_previous(&mut self) -> bool {
        let Some(find) = self.find.as_ref() else {
            return false;
        };
        self.core.execute_command(EditorCommand::Find {
            text: &find.query,
            case_sensitive: find.case_sensitive,
            direction: FindDirection::Previous,
        });
        self.core
            .find_status(&find.query, find.case_sensitive)
            .total
            > 0
    }

    /// Replaces the match the selection currently sits on, if any, and
    /// advances to the next one (see [`EditorCommand::ReplaceMatch`]).
    fn replace_current(&mut self) {
        let Some(find) = self.find.as_ref() else {
            return;
        };
        self.core.execute_command(EditorCommand::ReplaceMatch {
            text: &find.query,
            case_sensitive: find.case_sensitive,
            replacement: find.replace.as_bytes(),
        });
    }

    /// Replaces every match (see [`EditorCommand::ReplaceAllMatches`]).
    fn replace_all(&mut self) {
        let Some(find) = self.find.as_ref() else {
            return;
        };
        self.core.execute_command(EditorCommand::ReplaceAllMatches {
            text: &find.query,
            case_sensitive: find.case_sensitive,
            replacement: find.replace.as_bytes(),
        });
    }

    /// Renders the find/replace bar and dispatches interactions. Sets
    /// `self.find_reveal` when the visible selection moved and the view
    /// should scroll to follow it.
    fn find_bar(&mut self, ui: &mut egui::Ui) {
        if ui.input(|input| input.key_pressed(Key::Escape)) {
            self.close_find();
            return;
        }
        let Some(find) = self.find.as_mut() else {
            return;
        };
        let status = self.core.find_status(&find.query, find.case_sensitive);
        let mut changed = false;
        let mut go_next = false;
        let mut go_previous = false;
        let mut do_replace = false;
        let mut do_replace_all = false;
        let mut close = false;
        ui.horizontal(|ui| {
            ui.label(ICON_SEARCH);
            let response = ui.add(
                egui::TextEdit::singleline(&mut find.query)
                    .desired_width(160.0)
                    .hint_text("Find"),
            );
            if find.focus_query {
                response.request_focus();
                find.focus_query = false;
            }
            changed |= response.changed();
            if response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter)) {
                if ui.input(|input| input.modifiers.shift) {
                    go_previous = true;
                } else {
                    go_next = true;
                }
                response.request_focus();
            }
            if ui
                .selectable_label(find.case_sensitive, ICON_MATCH_CASE)
                .on_hover_text("Match case")
                .clicked()
            {
                find.case_sensitive = !find.case_sensitive;
                changed = true;
            }
            ui.weak(match (status.total, status.current) {
                (0, _) if find.query.is_empty() => String::new(),
                (0, _) => "No results".to_owned(),
                (total, Some(current)) => format!("{}/{total}", current + 1),
                (total, None) => format!("0/{total}"),
            });
            if ui
                .add_enabled(status.total > 0, egui::Button::new(ICON_KEYBOARD_ARROW_UP))
                .on_hover_text("Previous match")
                .clicked()
            {
                go_previous = true;
            }
            if ui
                .add_enabled(
                    status.total > 0,
                    egui::Button::new(ICON_KEYBOARD_ARROW_DOWN),
                )
                .on_hover_text("Next match")
                .clicked()
            {
                go_next = true;
            }
            if ui.button(ICON_CLOSE).on_hover_text("Close").clicked() {
                close = true;
            }
        });
        if find.show_replace {
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut find.replace)
                        .desired_width(160.0)
                        .hint_text("Replace"),
                );
                if ui
                    .add_enabled(status.current.is_some(), egui::Button::new("Replace"))
                    .clicked()
                {
                    do_replace = true;
                }
                if ui
                    .add_enabled(status.total > 0, egui::Button::new("Replace All"))
                    .clicked()
                {
                    do_replace_all = true;
                }
            });
        }
        if changed {
            self.find_reveal |= self.sync_find();
        }
        if go_next {
            self.find_reveal |= self.find_next();
        }
        if go_previous {
            self.find_reveal |= self.find_previous();
        }
        if do_replace {
            self.replace_current();
            self.find_reveal = true;
        }
        if do_replace_all {
            self.replace_all();
        }
        if close {
            self.close_find();
        }
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
            Key::F if modifiers.command => self.open_find(false),
            Key::H if modifiers.command => self.open_find(true),
            Key::D if modifiers.command && modifiers.shift => self
                .core
                .execute_command(EditorCommand::DuplicateLine(UDDirection::Down)),
            Key::D if modifiers.command => self
                .core
                .execute_command(EditorCommand::DuplicateCursor(LRDirection::Right)),
            Key::OpenBracket if modifiers.command && modifiers.shift => {
                self.core.execute_command(if self.cursor_line_collapsed() {
                    EditorCommand::Uncollapse
                } else {
                    EditorCommand::Collapse
                });
            }
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
        let (pressed, down, time, modifiers, touch_screen) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.time,
                input.modifiers,
                input.any_touches(),
            )
        });
        if pressed {
            self.touch_mode = touch_screen;
        }
        let Some(pointer) = response.interact_pointer_pos() else {
            if !down {
                self.selecting = false;
                self.dragging_handle = None;
                self.dragging_handle_offset = Vec2::ZERO;
            }
            return false;
        };
        let local_pointer = pointer - origin;
        if self.touch_mode {
            if self.dragging_handle.is_some() {
                if down {
                    self.drag_selection_handle(layout, local_pointer);
                    return true;
                }
                self.dragging_handle = None;
                self.dragging_handle_offset = Vec2::ZERO;
            } else if pressed {
                if let Some(handle) = self.selection_handle_at(layout, local_pointer) {
                    self.begin_selection_handle_drag(handle, layout, local_pointer);
                    self.touch_menu_open = false;
                    return true;
                }
            }
        }
        if layout.embeds.iter().any(|embed| {
            embed.large && embed.available && embed.rect.contains(local_pointer.to_pos2())
        }) {
            self.selecting = false;
            return false;
        }
        if touch_screen && !pressed && down {
            // A finger dragging across the editor on a touch screen scrolls
            // the view rather than extending the text selection.
            self.selecting = false;
            let delta = ui.input(|input| input.pointer.delta());
            if delta != Vec2::ZERO {
                ui.scroll_with_delta(delta);
            }
            return true;
        }
        let target = hit_test(layout, local_pointer);
        let tapped = if self.touch_mode {
            response.clicked()
        } else {
            pressed
        };
        if tapped && response.contains_pointer() {
            response.request_focus();
            if self.touch_mode && self.selection_contains_byte(target) {
                self.touch_menu_pos = pointer;
                self.touch_menu_open = true;
                self.selecting = false;
                self.click_count = 0;
                self.last_click = None;
                return true;
            }
            if let Some(checkbox) = checkbox_at(layout, checkboxes, local_pointer.to_pos2()) {
                self.core.execute_command(EditorCommand::Markdown(
                    MarkdownCommand::ToggleCheckbox(self.core.position(checkbox.line_start)),
                ));
                self.selecting = false;
                return true;
            }
            if let Some(embed) = layout
                .embeds
                .iter()
                .find(|embed| !embed.large && embed.rect.contains(local_pointer.to_pos2()))
            {
                self.core.execute_command(EditorCommand::SetSelection {
                    anchor: self.core.position(embed.range.start),
                    focus: self.core.position(embed.range.end),
                });
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

    fn selection_contains_byte(&self, byte: usize) -> bool {
        let Some(cursor) = self.core.cursor_positions().first() else {
            return false;
        };
        let Some(range) = self.core.selection_range(cursor) else {
            return false;
        };
        range.start < range.end && byte >= range.start && byte < range.end
    }

    fn selection_handle_at(
        &self,
        layout: &DocumentLayout,
        local_pointer: Vec2,
    ) -> Option<SelectionHandle> {
        let cursor = self.core.cursor_positions().first()?;
        let range = self.core.selection_range(cursor)?;
        if range.start == range.end {
            return None;
        }
        let start = touch_handle_center(
            touch_handle_anchor(layout, range.start)?,
            SelectionHandle::Start,
        );
        let end = touch_handle_center(
            touch_handle_anchor(layout, range.end)?,
            SelectionHandle::End,
        );
        if (local_pointer - start).length() <= TOUCH_HANDLE_HIT_RADIUS {
            Some(SelectionHandle::Start)
        } else if (local_pointer - end).length() <= TOUCH_HANDLE_HIT_RADIUS {
            Some(SelectionHandle::End)
        } else {
            None
        }
    }

    fn begin_selection_handle_drag(
        &mut self,
        handle: SelectionHandle,
        layout: &DocumentLayout,
        local_pointer: Vec2,
    ) {
        let Some(cursor) = self.core.cursor_positions().first() else {
            return;
        };
        let Some(range) = self.core.selection_range(cursor) else {
            return;
        };
        let (fixed_byte, moving_byte) = match handle {
            SelectionHandle::Start => (range.end, range.start),
            SelectionHandle::End => (range.start, range.end),
        };
        self.dragging_handle = Some(self.core.position(fixed_byte));
        self.dragging_handle_offset = touch_handle_anchor(layout, moving_byte)
            .map_or(Vec2::ZERO, |anchor| local_pointer - anchor);
        self.selecting = false;
        self.drag_selection_handle(layout, local_pointer);
    }

    fn drag_selection_handle(&mut self, layout: &DocumentLayout, local_pointer: Vec2) {
        let Some(fixed) = self.dragging_handle else {
            return;
        };
        let target = hit_test(layout, local_pointer - self.dragging_handle_offset);
        self.core
            .execute_command(EditorCommand::DragSelectionHandle {
                fixed,
                position: self.core.position(target),
            });
    }

    fn paint_touch_handles(
        &self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
    ) {
        if !self.touch_mode {
            return;
        }
        let Some(cursor) = self.core.cursor_positions().first() else {
            return;
        };
        let Some(range) = self.core.selection_range(cursor) else {
            return;
        };
        if range.start == range.end {
            return;
        }
        let color = ui.visuals().selection.stroke.color;
        for (byte, handle) in [
            (range.start, SelectionHandle::Start),
            (range.end, SelectionHandle::End),
        ] {
            let Some(anchor) = touch_handle_anchor(layout, byte) else {
                continue;
            };
            let (rect, corner_radius) = touch_handle_shape(anchor, handle);
            painter.rect_filled(rect.translate(origin.to_vec2()), corner_radius, color);
        }
    }

    fn touch_selection_menu(&mut self, response: &egui::Response) {
        if !self.touch_menu_open {
            return;
        }
        let mut open = true;
        egui::Popup::from_response(response)
            .id(egui::Id::new((self.block.id(), "touch-selection-menu")))
            .at_position(self.touch_menu_pos)
            .open_bool(&mut open)
            .show(|ui| {
                if ui.button("Copy").clicked() {
                    let text = self.core.copy_utf8(CopyMode::Copy);
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                    ui.close();
                }
                if ui.button("Cut").clicked() {
                    let text = self.core.copy_utf8(CopyMode::Cut);
                    if !text.is_empty() {
                        ui.ctx().copy_text(text);
                    }
                    ui.close();
                }
                if ui.button("Select All").clicked() {
                    self.core.execute_command(EditorCommand::SelectAll);
                    ui.close();
                }
            });
        self.touch_menu_open = open;
    }

    fn paint(
        &mut self,
        ui: &egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
        highlight: &SyntaxHighlight,
        focused: bool,
    ) -> (Option<Rect>, Vec<Rect>, PaintTimings) {
        let selection_start = Instant::now();
        paint_code_backgrounds(painter, origin, layout, highlight);
        let selection_color = ui.visuals().selection.bg_fill;
        let mut cursor_rect = None;
        let mut cursor_rects = Vec::new();
        let mut selected_bytes = vec![false; layout.positions.len().saturating_sub(1)];
        for cursor in self.core.cursor_positions() {
            let Some(Range { start, end }) = self.core.selection_range(cursor) else {
                continue;
            };
            let Some(focus) = self.core.position_index(cursor.pos.focus) else {
                continue;
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
            if focused {
                let Some(position) = layout.positions.get(focus).and_then(|position| *position)
                else {
                    continue;
                };
                let top = Pos2::new(
                    origin.x + position.x,
                    origin.y + layout.lines[position.line].y,
                );
                let rect =
                    Rect::from_min_size(top, Vec2::new(2.0, layout.lines[position.line].height));
                cursor_rects.push(rect);
                cursor_rect = Some(rect);
            }
        }

        let selection = selection_start.elapsed();
        let renderer = match &mut self.renderer {
            Ok(renderer) => renderer,
            Err(_) => {
                return (
                    cursor_rect,
                    cursor_rects,
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
            cursor_rects,
            PaintTimings {
                selection,
                glyphs: glyph_start.elapsed(),
                rasterize,
                glyph_count,
                cache_misses,
            },
        )
    }

    /// Paints other clients' cursors and selections in this block, in the
    /// color each one is shown with in the "Also viewing" indicator. A
    /// remote cursor whose color hasn't arrived yet (a brief race on the
    /// first frame it appears) is skipped rather than shown in a fallback
    /// color, since the indicator would otherwise disagree with it.
    fn paint_remote_cursors(
        &self,
        client: &BlockClient,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
    ) {
        let colors: HashMap<ClientId, PresenceColor> = client
            .presence::<UserActive>(self.block.id())
            .into_iter()
            .map(|(client_id, user)| (client_id, user.color))
            .collect();
        for (client_id, cursor) in client.presence::<TextCursor>(self.block.id()) {
            let Some(color) = colors.get(&client_id).copied() else {
                continue;
            };
            let Some(Range { start, end }) = self
                .core
                .selection_range(&CursorPosition::range(cursor.anchor, cursor.focus))
            else {
                continue;
            };
            let Some(focus) = self.core.position_index(cursor.focus) else {
                continue;
            };
            let rgb = presence_color_rgb(color);
            let [red, green, blue, _] = rgb.to_srgba_unmultiplied();
            let fill = Color32::from_rgba_unmultiplied(red, green, blue, 70);
            for byte in start..end {
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
                    fill,
                );
            }
            let Some(position) = layout.positions.get(focus).and_then(|position| *position) else {
                continue;
            };
            let top = Pos2::new(
                origin.x + position.x,
                origin.y + layout.lines[position.line].y,
            );
            painter.rect_filled(
                Rect::from_min_size(top, Vec2::new(2.0, layout.lines[position.line].height)),
                0.0,
                rgb,
            );
            painter.rect_filled(
                Rect::from_min_size(top - Vec2::new(0.0, 4.0), Vec2::new(6.0, 4.0)),
                0.0,
                rgb,
            );
        }
    }

    /// Screen-space rect of the given client's cursor, for scrolling it into
    /// view when its presence indicator is clicked. Mirrors the caret
    /// geometry painted by [`Self::paint_remote_cursors`].
    fn presence_cursor_rect(
        &self,
        client: &BlockClient,
        client_id: ClientId,
        origin: Pos2,
        layout: &DocumentLayout,
    ) -> Option<Rect> {
        let cursor = client
            .presence::<TextCursor>(self.block.id())
            .into_iter()
            .find(|(id, _)| *id == client_id)?
            .1;
        let focus = self.core.position_index(cursor.focus)?;
        let position = layout.positions.get(focus).and_then(|position| *position)?;
        let top = Pos2::new(
            origin.x + position.x,
            origin.y + layout.lines[position.line].y,
        );
        Some(Rect::from_min_size(
            top,
            Vec2::new(2.0, layout.lines[position.line].height),
        ))
    }

    fn paint_embeds(
        &mut self,
        ui: &mut egui::Ui,
        painter: &egui::Painter,
        origin: Pos2,
        layout: &DocumentLayout,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        if self.focused_embed.is_some_and(|focused| {
            !layout.embeds.iter().any(|embed| {
                embed.large && embed.id == focused.id && embed.range.start == focused.source_start
            })
        }) {
            self.focused_embed = None;
        }
        let mut action = None;
        for embed in &layout.embeds {
            let rect = embed.rect.translate(origin.to_vec2());
            if embed.large {
                let inner = rect.shrink(EMBEDDED_EDITOR_PADDING);
                let title_bar = Rect::from_min_size(
                    inner.min,
                    Vec2::new(inner.width(), EMBEDDED_EDITOR_TITLE_HEIGHT),
                );
                let content = Rect::from_min_max(
                    Pos2::new(inner.left(), title_bar.bottom() + EMBEDDED_EDITOR_TITLE_GAP),
                    inner.max,
                );
                painter.rect(
                    rect,
                    6.0,
                    ui.visuals().panel_fill,
                    egui::Stroke::new(1.0_f32, ui.visuals().widgets.noninteractive.bg_stroke.color),
                    egui::StrokeKind::Inside,
                );
                painter.rect_filled(title_bar, 4.0, ui.visuals().widgets.inactive.bg_fill);

                let mut title_x = title_bar.left() + 6.0;
                if let Some(icon) = embed.icon {
                    painter.text(
                        Pos2::new(title_x, title_bar.center().y),
                        egui::Align2::LEFT_CENTER,
                        icon,
                        egui::FontId::new(
                            16.0,
                            egui::FontFamily::Name(egui_material_icons::FONT_FAMILY.into()),
                        ),
                        ui.visuals().text_color(),
                    );
                    title_x += 22.0;
                }
                paint_name(
                    &painter.with_clip_rect(title_bar),
                    Pos2::new(title_x, title_bar.center().y),
                    egui::Align2::LEFT_CENTER,
                    &embed.label,
                    egui::FontId::proportional(16.0),
                    ui.visuals().text_color(),
                    embed.automatic,
                );

                let key = FocusedEmbed {
                    id: embed.id,
                    source_start: embed.range.start,
                };
                let focused = self.focused_embed == Some(key);
                let interaction = editors
                    .direct_editor_interaction(embed.id)
                    .unwrap_or(DirectEditorInteraction::Preview);
                let show_editor = interaction != DirectEditorInteraction::Preview || focused;
                if interaction == DirectEditorInteraction::Preview && !focused && embed.available {
                    let button_rect = Rect::from_min_size(
                        Pos2::new(title_bar.right() - 54.0, title_bar.top() + 2.0),
                        Vec2::new(52.0, title_bar.height() - 4.0),
                    );
                    if ui.put(button_rect, egui::Button::new("Edit")).clicked() {
                        self.focused_embed = Some(key);
                    }
                }
                if interaction == DirectEditorInteraction::Live
                    && ui.ctx().input(|input| {
                        input.pointer.button_pressed(PointerButton::Primary)
                            && input
                                .pointer
                                .interact_pos()
                                .is_some_and(|pointer| content.contains(pointer))
                    })
                {
                    self.focused_embed = Some(key);
                }

                if embed.available && show_editor {
                    let embedded = embedded_editor_ui(
                        ui,
                        editors,
                        embed.id,
                        ("text-direct-editor", self.block.id(), embed.range.start),
                        content,
                        content.intersect(ui.clip_rect()),
                        1.0,
                        viewport,
                    );
                    action = action.or(embedded);
                } else if embed.available {
                    let rendered = editors.render(
                        embed.id,
                        BlockRenderContext {
                            painter,
                            corners: [
                                content.left_top(),
                                content.right_top(),
                                content.right_bottom(),
                                content.left_bottom(),
                            ],
                            opacity: 1.0,
                        },
                    );
                    if !rendered {
                        painter.rect_filled(content, 0.0, Color32::from_gray(35));
                    }
                } else {
                    let title = painter.layout_no_wrap(
                        "Block unavailable".to_owned(),
                        egui::FontId::proportional(13.0),
                        ui.visuals().weak_text_color(),
                    );
                    painter.galley(
                        content.center() - title.size() * 0.5,
                        title,
                        ui.visuals().weak_text_color(),
                    );
                }
                continue;
            }
            let selected = self.core.cursor_positions().iter().any(|cursor| {
                let Some(Range { start, end }) = self.core.selection_range(cursor) else {
                    return false;
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
        action
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
            let Some(rect) = checkbox_rect(layout, checkbox) else {
                continue;
            };
            let rect = rect.translate(origin.to_vec2());
            painter.rect(
                rect,
                3.0,
                if checkbox.checked {
                    ui.visuals().selection.bg_fill
                } else {
                    Color32::TRANSPARENT
                },
                egui::Stroke::new(1.5_f32, ui.visuals().widgets.inactive.fg_stroke.color),
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
        let selection = self.core.selection_range(cursor)?;
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

    fn direct_editor_size(&mut self, editors: &mut EditorAccess<'_>, width: f32) -> Option<Vec2> {
        let bytes = self.block.read()?.bytes().to_vec();
        let highlight = self.core.highlight();
        let embeds = self.resolve_embeds(&bytes, editors);
        let checkboxes = if self.core.language() == TextLanguage::Markdown {
            parse_markdown_checkboxes(&bytes)
                .iter()
                .map(|checkbox| checkbox.marker.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let hidden = hidden_ranges_from_sections(&self.core.collapsible_sections());
        let height = match &self.renderer {
            Ok(renderer) => {
                renderer
                    .layout_profiled(
                        &bytes,
                        &highlight,
                        &embeds,
                        &checkboxes,
                        &hidden,
                        (width - PADDING.x * 2.0).max(1.0),
                    )
                    .0
                    .size
                    .y
            }
            Err(_) => PADDING.y * 2.0,
        };
        Some(Vec2::new(width, height))
    }
}

/// The byte ranges hidden by the document's currently-collapsed sections,
/// for [`font::TextRenderer::layout_profiled`] to skip over.
fn hidden_ranges_from_sections(sections: &[CollapsibleSection]) -> Vec<Range<usize>> {
    sections
        .iter()
        .filter(|section| section.collapsed)
        .map(|section| section.line_end + 1..section.content_end + 1)
        .collect()
}

impl BlockEditor for TextEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        let mut core = Core::new(self.block.clone());
        core.execute_command(EditorCommand::ReplaceBlockReference { old, new: new.id });
        Some(true)
    }

    fn sync_cursor_presence(&mut self, client: &BlockClient, visible: bool) {
        if !visible {
            client.set_presence::<TextCursor>(self.block.id(), None);
            return;
        }
        let Some(cursor) = self.core.cursor_positions().first() else {
            return;
        };
        client.set_presence(
            self.block.id(),
            Some(&TextCursor {
                anchor: cursor.pos.anchor,
                focus: cursor.pos.focus,
            }),
        );
    }

    fn reveal_presence_cursor(&mut self, client_id: ClientId) {
        self.pending_presence_reveal = Some(client_id);
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
        DirectEditorResize::Horizontal
    }

    fn direct_editor_intrinsic_size(&mut self, editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        self.direct_editor_size(editors, DIRECT_EDITOR_WIDTH)
    }

    fn direct_editor_intrinsic_size_for_width(
        &mut self,
        width: f32,
        editors: &mut EditorAccess<'_>,
    ) -> Option<Vec2> {
        self.direct_editor_size(editors, width)
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        if let Some(focused) = self.focused_embed {
            ui.horizontal(|ui| {
                if ui
                    .button(format!("{} Back", ICON_ARROW_BACK.codepoint))
                    .clicked()
                {
                    self.focused_embed = None;
                }
            });
            return self
                .focused_embed
                .and_then(|_| editors.direct_editor_top_bar(focused.id, ui, viewport));
        }
        let toolbar_start = Instant::now();
        self.toolbar(ui, editors);
        if self.find.is_some() {
            self.find_bar(ui);
        }
        if let Some(error) = self.image_import_error.clone() {
            ui.horizontal(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, error);
                if ui.small_button("Dismiss").clicked() {
                    self.image_import_error = None;
                }
            });
        }
        self.toolbar_profile = toolbar_start.elapsed();
        None
    }

    fn direct_editor_has_left_sidebar(&self, editors: &mut EditorAccess<'_>) -> bool {
        self.focused_embed
            .is_some_and(|focused| editors.direct_editor_has_left_sidebar(focused.id))
    }

    fn direct_editor_left_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.focused_embed
            .and_then(|focused| editors.direct_editor_left_sidebar(focused.id, ui))
    }

    fn direct_editor_has_right_sidebar(&self, editors: &mut EditorAccess<'_>) -> bool {
        self.focused_embed
            .is_some_and(|focused| editors.direct_editor_has_right_sidebar(focused.id))
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        self.focused_embed
            .and_then(|focused| editors.direct_editor_right_sidebar(focused.id, ui))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let _performance_group =
            performance::GroupGuard::new(format!("Text editor ({})", self.block.id()));
        let frame_start = Instant::now();
        let mut profile = FrameProfile {
            toolbar: std::mem::take(&mut self.toolbar_profile),
            ..FrameProfile::default()
        };
        let id = egui::Id::new(("text-editor", self.block.id()));
        let keyboard_start = Instant::now();
        let pasted_image = self.paste_clipboard_image(ui, id, editors);
        let mut reveal_cursor = pasted_image || self.keyboard_input(ui, id, pasted_image);
        reveal_cursor |= std::mem::take(&mut self.find_reveal);
        profile.keyboard = keyboard_start.elapsed();
        self.handle_picker(ui.ctx(), editors);
        let document_start = Instant::now();
        let Some(bytes) = self.block.read().map(|document| document.bytes().to_vec()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            profile.document = document_start.elapsed();
            profile.total = frame_start.elapsed() + profile.toolbar;
            record_profile(profile);
            return None;
        };
        profile.document = document_start.elapsed();
        profile.document_bytes = bytes.len();
        let language = self.core.language();
        let checkboxes = if language == TextLanguage::Markdown {
            parse_markdown_checkboxes(&bytes)
        } else {
            Vec::new()
        };
        let checkbox_marker_ranges = checkboxes
            .iter()
            .map(|checkbox| checkbox.marker.clone())
            .collect::<Vec<_>>();
        let sections = self.core.collapsible_sections();
        let hidden = hidden_ranges_from_sections(&sections);
        let total_lines = bytes.iter().filter(|byte| **byte == b'\n').count() + 1;
        let wrap_width = (ui.available_width() - gutter_width(ui, total_lines) - PADDING.x * 2.0)
            .max(1.0)
            .round();
        let highlight_start = Instant::now();
        let highlight = self.core.highlight();
        profile.highlight = highlight_start.elapsed();
        let layout_start = Instant::now();
        let embeds = self.resolve_embeds(&bytes, editors);
        let layout = if let Some(cached) = self.layout_cache.as_ref().filter(|cached| {
            cached.language == language
                && cached.bytes == bytes
                && cached.embeds == embeds
                && cached.hidden == hidden
                && cached.wrap_width == wrap_width
        }) {
            Arc::clone(&cached.layout)
        } else {
            let layout = match &self.renderer {
                Ok(renderer) => {
                    let (layout, detail) = renderer.layout_profiled(
                        &bytes,
                        &highlight,
                        &embeds,
                        &checkbox_marker_ranges,
                        &hidden,
                        wrap_width,
                    );
                    profile.layout_detail = Some(detail);
                    Arc::new(layout)
                }
                Err(error) => {
                    ui.centered_and_justified(|ui| {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    });
                    profile.layout = layout_start.elapsed();
                    profile.total = frame_start.elapsed() + profile.toolbar;
                    record_profile(profile);
                    return None;
                }
            };
            self.layout_cache = Some(CachedLayout {
                bytes,
                language,
                embeds,
                hidden,
                wrap_width,
                layout: Arc::clone(&layout),
            });
            layout
        };
        profile.layout = layout_start.elapsed();
        profile.line_count = layout.lines.len();

        let gutter_width = gutter_width(ui, layout.total_lines);
        let desired =
            Vec2::new(layout.size.x + gutter_width, layout.size.y).max(ui.available_size());
        let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
        let response = ui.interact(rect, id, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        let response = response.on_hover_cursor(egui::CursorIcon::Text);
        ui.painter()
            .rect_filled(response.rect, 0.0, Color32::from_rgb(29, 37, 44));
        let origin = response.rect.min + Vec2::new(gutter_width, 0.0) + PADDING;
        paint_gutter(
            &painter,
            response.rect,
            gutter_width,
            origin.y,
            &layout,
            &sections,
        );
        paint_revealed_backgrounds(&painter, response.rect, origin, &layout, &sections);
        let edit_block = self.selected_embed_action(ui.ctx(), origin, &layout, editors.client());
        let pointer_start = Instant::now();
        let gutter_arrow_click = ui
            .input(|input| input.pointer.button_pressed(PointerButton::Primary))
            .then(|| response.interact_pointer_pos())
            .flatten()
            .and_then(|pointer| {
                gutter_arrow_at(
                    response.rect,
                    gutter_width,
                    origin.y,
                    &layout,
                    &sections,
                    pointer,
                )
            });
        if let Some(line_start) = gutter_arrow_click {
            self.core.execute_command(EditorCommand::ToggleCollapseAt(
                self.core.position(line_start),
            ));
        } else {
            reveal_cursor |= self.pointer_input(ui, &response, origin, &layout, &checkboxes);
        }
        profile.pointer = pointer_start.elapsed();
        if let Some(hover) = response.hover_pos() {
            if checkbox_at(&layout, &checkboxes, (hover - origin).to_pos2()).is_some()
                || gutter_arrow_at(
                    response.rect,
                    gutter_width,
                    origin.y,
                    &layout,
                    &sections,
                    hover,
                )
                .is_some()
            {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
        let embedded_action = self.paint_embeds(ui, &painter, origin, &layout, editors, viewport);
        let pointer = response
            .interact_pointer_pos()
            .or_else(|| response.ctx.pointer_hover_pos());
        let drop_index = response
            .dnd_hover_payload::<SidebarDragPayload>()
            .filter(|dragged| dragged.reference.id != self.block.id())
            .and(pointer)
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
                    self.core.execute_command(EditorCommand::SetSelection {
                        anchor: position,
                        focus: position,
                    });
                    let name =
                        BlockLabel::for_reference(editors.registry(), &dragged.reference).name;
                    self.insert_image_embed(dragged.reference.id, &name);
                    reveal_cursor = true;
                }
            }
        }
        let (cursor, caret_rects, paint) = self.paint(
            ui,
            &painter,
            origin,
            &layout,
            &highlight,
            response.has_focus(),
        );
        self.paint_remote_cursors(editors.client(), &painter, origin, &layout);
        if let Some(client_id) = std::mem::take(&mut self.pending_presence_reveal) {
            if let Some(rect) =
                self.presence_cursor_rect(editors.client(), client_id, origin, &layout)
            {
                ui.scroll_to_rect(rect.expand2(Vec2::new(8.0, 3.0)), Some(egui::Align::Center));
            }
        }
        self.paint_checkboxes(ui, &painter, origin, &layout, &checkboxes);
        paint_collapsed_ellipsis(&painter, origin, &layout, &sections);
        let cursor_color = ui.visuals().selection.stroke.color;
        for rect in caret_rects {
            painter.rect_filled(rect, 0.0, cursor_color);
        }
        self.paint_touch_handles(ui, &painter, origin, &layout);
        self.touch_selection_menu(&response);
        if response.has_focus() {
            report_ime_area(ui, response.rect, cursor);
        }
        profile.paint = paint;
        let cursor_positions = self.core.cursor_positions().to_vec();
        if reveal_cursor && cursor_positions != self.last_cursor_positions {
            if let Some(cursor) = cursor {
                ui.scroll_to_rect(cursor.expand2(Vec2::new(8.0, 3.0)), None);
            }
        }
        self.last_cursor_positions = cursor_positions;
        profile.total = frame_start.elapsed() + profile.toolbar;
        record_profile(profile);
        edit_block.or(embedded_action)
    }
}

/// Publishes the focused editor area as this frame's IME target. The backend
/// only allows IME while some widget reports one, and on Android allowing IME
/// is what raises the on-screen keyboard.
fn report_ime_area(ui: &egui::Ui, rect: Rect, cursor: Option<Rect>) {
    let to_global = ui
        .ctx()
        .layer_transform_to_global(ui.layer_id())
        .unwrap_or_default();
    let cursor = cursor.unwrap_or_else(|| Rect::from_min_size(rect.min + PADDING, Vec2::X));
    ui.output_mut(|output| {
        output.ime = Some(IMEOutput {
            rect: to_global * rect,
            cursor_rect: to_global * cursor,
        });
    });
}

fn record_profile(profile: FrameProfile) {
    for (id, duration) in [
        ("Frame total", profile.total),
        ("Keyboard input", profile.keyboard),
        ("Toolbar", profile.toolbar),
        ("Document read + copy", profile.document),
        ("Syntax highlight", profile.highlight),
        ("Layout total", profile.layout),
        ("Pointer hit testing", profile.pointer),
        ("Selection + cursor paint", profile.paint.selection),
        ("Glyph paint", profile.paint.glyphs),
    ] {
        performance::record_duration(id, duration);
    }
    if let Some(layout) = profile.layout_detail {
        for (id, duration) in [
            ("Display-line conversion", layout.display_lines),
            ("Font/style run detection", layout.font_runs),
            ("HarfBuzz shaping", layout.shape),
            ("Line positions + metrics", layout.line_finalize),
            ("Markdown table alignment", layout.tables),
        ] {
            performance::record_duration(id, duration);
        }
    }
    if profile.paint.cache_misses != 0 {
        performance::record_duration("Glyph rasterization", profile.paint.rasterize);
    }
    performance::record_count("Document bytes", profile.document_bytes as u64);
    performance::record_count("Lines", profile.line_count as u64);
    performance::record_count("Glyphs visited", profile.paint.glyph_count as u64);
    performance::record_count("Glyph cache misses", profile.paint.cache_misses as u64);
}

fn image_embed_directive(
    workspace_id: Uuid,
    id: Uuid,
    source_name: &str,
    markdown: bool,
) -> String {
    let url = block_url(workspace_id, id);
    if markdown {
        format!("![{source_name}]({url})")
    } else {
        url
    }
}

fn parse_embeds(bytes: &[u8], workspace_id: Uuid, markdown: bool) -> Vec<ParsedEmbed> {
    parse_block_urls(bytes)
        .into_iter()
        .filter(|url| url.workspace_id == workspace_id)
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
        if let Some(marker) = markdown_checkbox_marker(bytes, line_start) {
            result.push(MarkdownCheckbox {
                line_start,
                marker: marker.marker,
                checked: marker.checked,
            });
        }
        if line_end == bytes.len() {
            break;
        }
        line_start = line_end + 1;
    }
    result
}

/// The full reserved layout column for a checkbox marker (the whole
/// `- [ ]`), used as the click/hover target so the checkbox covers that
/// entire span rather than just the drawn box.
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
            Pos2::new(right.x, line.y + line.height),
        )
    })
}

/// The drawn checkbox box: a square centered in [`checkbox_marker_rect`].
fn checkbox_rect(layout: &DocumentLayout, checkbox: &MarkdownCheckbox) -> Option<Rect> {
    let marker = checkbox_marker_rect(layout, checkbox)?;
    let size = marker.width().min(marker.height());
    Some(Rect::from_center_size(marker.center(), Vec2::splat(size)))
}

fn checkbox_at<'a>(
    layout: &DocumentLayout,
    checkboxes: &'a [MarkdownCheckbox],
    point: Pos2,
) -> Option<&'a MarkdownCheckbox> {
    checkboxes.iter().find(|checkbox| {
        checkbox_marker_rect(layout, checkbox).is_some_and(|rect| rect.contains(point))
    })
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

fn touch_handle_anchor(layout: &DocumentLayout, byte: usize) -> Option<Vec2> {
    let position = layout.positions.get(byte).and_then(|position| *position)?;
    let line = layout.lines.get(position.line)?;
    Some(Vec2::new(
        position.x,
        line.y + line.height + TOUCH_HANDLE_GAP,
    ))
}

fn touch_handle_shape(anchor: Vec2, handle: SelectionHandle) -> (Rect, CornerRadius) {
    let diameter = TOUCH_HANDLE_RADIUS * 2.0;
    let radius = TOUCH_HANDLE_RADIUS.round() as u8;
    let rect = match handle {
        SelectionHandle::Start => Rect::from_min_max(
            Pos2::new(anchor.x - diameter, anchor.y),
            Pos2::new(anchor.x, anchor.y + diameter),
        ),
        SelectionHandle::End => Rect::from_min_max(
            Pos2::new(anchor.x, anchor.y),
            Pos2::new(anchor.x + diameter, anchor.y + diameter),
        ),
    };
    let corner_radius = match handle {
        SelectionHandle::Start => CornerRadius {
            nw: radius,
            ne: 0,
            sw: radius,
            se: radius,
        },
        SelectionHandle::End => CornerRadius {
            nw: 0,
            ne: radius,
            sw: radius,
            se: radius,
        },
    };
    (rect, corner_radius)
}

fn touch_handle_center(anchor: Vec2, handle: SelectionHandle) -> Vec2 {
    match handle {
        SelectionHandle::Start => anchor + Vec2::new(-TOUCH_HANDLE_RADIUS, TOUCH_HANDLE_RADIUS),
        SelectionHandle::End => anchor + Vec2::new(TOUCH_HANDLE_RADIUS, TOUCH_HANDLE_RADIUS),
    }
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

fn paint_code_backgrounds(
    painter: &egui::Painter,
    origin: Pos2,
    layout: &DocumentLayout,
    highlight: &SyntaxHighlight,
) {
    let fill = Color32::from_rgb(23, 30, 36);
    for line in &layout.lines {
        let mut start = None;
        for byte in line.start..=line.end {
            let is_code =
                byte < line.end && highlight.style_at(byte).color == SynHlColorScope::MarkdownCode;
            if is_code {
                start.get_or_insert(byte);
                continue;
            }
            let Some(run_start) = start.take() else {
                continue;
            };
            let Some(left) = layout
                .positions
                .get(run_start)
                .and_then(|position| *position)
            else {
                continue;
            };
            let right = layout
                .positions
                .get(byte)
                .and_then(|position| *position)
                .filter(|right| right.line == left.line)
                .unwrap_or(BytePosition {
                    line: left.line,
                    x: line.width,
                });
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(origin.x + left.x - 3.0, origin.y + line.y + 1.0),
                    Pos2::new(
                        origin.x + right.x + 3.0,
                        origin.y + line.y + line.height - 1.0,
                    ),
                ),
                3.0,
                fill,
            );
        }
    }
}

fn gutter_digits(line_count: usize) -> usize {
    let mut digits = 1;
    let mut value = line_count.max(1);
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn gutter_width(ui: &egui::Ui, line_count: usize) -> f32 {
    let font_id = egui::FontId::monospace(GUTTER_TEXT_SIZE);
    let digit_width = ui.fonts_mut(|fonts| fonts.glyph_width(&font_id, '0'));
    GUTTER_PADDING_LEFT
        + GUTTER_ARROW_SIZE
        + digit_width * gutter_digits(line_count) as f32
        + GUTTER_PADDING_RIGHT
}

/// The collapsible section, if any, whose fold arrow is painted on `line`.
fn line_section<'a>(
    line: &LineLayout,
    sections: &'a [CollapsibleSection],
) -> Option<&'a CollapsibleSection> {
    sections
        .iter()
        .find(|section| section.line_start == line.start)
}

/// The fold-arrow hit rect for `line`, in the same coordinate space as
/// `rect` (the whole editor rect, gutter included).
fn gutter_arrow_rect(rect: Rect, text_top: f32, line: &LineLayout) -> Rect {
    let top = text_top + line.y;
    Rect::from_min_size(
        Pos2::new(
            rect.left() + GUTTER_PADDING_LEFT,
            top + (line.height - GUTTER_ARROW_SIZE) / 2.0,
        ),
        Vec2::splat(GUTTER_ARROW_SIZE),
    )
}

/// The collapsible line whose gutter fold arrow contains `pointer`, if any.
fn gutter_arrow_at(
    rect: Rect,
    gutter_width: f32,
    text_top: f32,
    layout: &DocumentLayout,
    sections: &[CollapsibleSection],
    pointer: Pos2,
) -> Option<usize> {
    if pointer.x < rect.left() || pointer.x > rect.left() + gutter_width {
        return None;
    }
    layout.lines.iter().find_map(|line| {
        let section = line_section(line, sections)?;
        gutter_arrow_rect(rect, text_top, line)
            .contains(pointer)
            .then_some(section.line_start)
    })
}

fn paint_gutter(
    painter: &egui::Painter,
    rect: Rect,
    gutter_width: f32,
    text_top: f32,
    layout: &DocumentLayout,
    sections: &[CollapsibleSection],
) {
    let gutter_rect = Rect::from_min_size(rect.min, Vec2::new(gutter_width, rect.height()));
    painter.rect_filled(gutter_rect, 0.0, Color32::from_rgb(24, 31, 37));
    painter.line_segment(
        [gutter_rect.right_top(), gutter_rect.right_bottom()],
        egui::Stroke::new(1.0_f32, Color32::from_rgb(40, 51, 60)),
    );
    let font_id = egui::FontId::monospace(GUTTER_TEXT_SIZE);
    let color = Color32::from_rgb(0x71, 0x8c, 0xa1);
    let arrow_color = Color32::from_rgb(0x8f, 0xa8, 0xba);
    let number_x = gutter_rect.right() - GUTTER_PADDING_RIGHT;
    for line in &layout.lines {
        if line.show_line_number {
            painter.text(
                Pos2::new(number_x, text_top + line.y + line.height / 2.0),
                egui::Align2::RIGHT_CENTER,
                line.document_line + 1,
                font_id.clone(),
                color,
            );
        }
        if let Some(section) = line_section(line, sections) {
            let icon = if section.collapsed || section.revealed {
                ICON_KEYBOARD_ARROW_RIGHT
            } else {
                ICON_KEYBOARD_ARROW_DOWN
            };
            painter.text(
                gutter_arrow_rect(rect, text_top, line).center(),
                egui::Align2::CENTER_CENTER,
                icon.codepoint,
                egui::FontId::new(GUTTER_ARROW_SIZE, icon.font_family()),
                arrow_color,
            );
        }
    }
}

/// Paints a background over the body of each collapsed section that's
/// temporarily revealed because a cursor sits inside it, so the lines that
/// would normally be hidden read as visually distinct from ordinary text.
fn paint_revealed_backgrounds(
    painter: &egui::Painter,
    rect: Rect,
    origin: Pos2,
    layout: &DocumentLayout,
    sections: &[CollapsibleSection],
) {
    let fill = Color32::from_rgb(45, 33, 20);
    for section in sections.iter().filter(|section| section.revealed) {
        let hidden_lines = layout
            .lines
            .iter()
            .filter(|line| line.start > section.line_end && line.start <= section.content_end);
        let Some(top) = hidden_lines.clone().map(|line| line.y).reduce(f32::min) else {
            continue;
        };
        let bottom = hidden_lines
            .map(|line| line.y + line.height)
            .reduce(f32::max)
            .unwrap_or(top);
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(rect.left(), origin.y + top),
                Pos2::new(rect.right(), origin.y + bottom),
            ),
            0.0,
            fill,
        );
    }
}

/// Paints a `...` right after the content of each collapsed line.
fn paint_collapsed_ellipsis(
    painter: &egui::Painter,
    origin: Pos2,
    layout: &DocumentLayout,
    sections: &[CollapsibleSection],
) {
    let font_id = egui::FontId::monospace(GUTTER_TEXT_SIZE);
    let color = Color32::from_rgb(0x8f, 0xa8, 0xba);
    for section in sections.iter().filter(|section| section.collapsed) {
        let Some(line) = layout
            .lines
            .iter()
            .find(|line| line.start == section.line_start)
        else {
            continue;
        };
        painter.text(
            Pos2::new(
                origin.x + line.width + COLLAPSED_ELLIPSIS_GAP,
                origin.y + line.y + line.height / 2.0,
            ),
            egui::Align2::LEFT_CENTER,
            "...",
            font_id.clone(),
            color,
        );
    }
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
