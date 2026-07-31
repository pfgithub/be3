mod font;

use block::{Block, BlockParent};
use block_client::{blocks::text::TextDocument, BlockClient, BlockHandle, BlockRelationships};
use eframe::egui::{
    self, Color32, Event, EventFilter, ImeEvent, Key, Modifiers, PointerButton, Pos2, Rect, Sense,
    Vec2,
};
use text_editor_core::{
    CopyMode, Core, CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode,
    EditorCommand, LRDirection, MoveMode, SynHlColorScope, SyntaxNodeDirection, UDDirection,
    VerticalMoveMode,
};
use uuid::Uuid;

use self::font::{BytePosition, DocumentLayout, TextRenderer, LINE_HEIGHT};
use super::{BlockEditor, EditorAccess, EditorAction, EditorRegistration};

const PADDING: Vec2 = Vec2::new(12.0, 8.0);

pub(super) fn registration() -> EditorRegistration {
    EditorRegistration {
        block_type: TextDocument::TYPE_ID,
        display_name: "Text",
        create: Some(|client| {
            let block = client.create_block(TextDocument::new());
            Box::new(TextEditor::new(block))
        }),
        open: |client: &BlockClient, id| {
            Box::new(TextEditor::new(client.get_block::<TextDocument>(id)))
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
}

impl TextEditor {
    fn new(block: BlockHandle<TextDocument>) -> Self {
        let mut core = Core::new(block.clone());
        core.execute_command(EditorCommand::SetCursorPosition(core.position(0)));
        Self {
            block,
            core,
            renderer: TextRenderer::new(),
            selecting: false,
        }
    }

    fn keyboard_input(&mut self, ui: &egui::Ui, id: egui::Id) -> bool {
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
    ) -> bool {
        let (pressed, down, double, triple, modifiers) = ui.input(|input| {
            (
                input.pointer.button_pressed(PointerButton::Primary),
                input.pointer.button_down(PointerButton::Primary),
                input.pointer.button_double_clicked(PointerButton::Primary),
                input.pointer.button_triple_clicked(PointerButton::Primary),
                input.modifiers,
            )
        });
        let Some(pointer) = response.interact_pointer_pos() else {
            if !down {
                self.selecting = false;
            }
            return false;
        };
        let target = hit_test(layout, pointer - origin);
        if pressed && response.contains_pointer() {
            response.request_focus();
            let mode = if triple {
                DragSelectionMode::select(CursorLeftRightStop::Line)
            } else if double {
                DragSelectionMode::select(CursorLeftRightStop::Word)
            } else {
                DragSelectionMode::default()
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
    ) -> Option<Rect> {
        let selection_color = ui.visuals().selection.bg_fill;
        let cursor_color = ui.visuals().selection.stroke.color;
        let mut cursor_rect = None;
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
                            y + LINE_HEIGHT,
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
                let rect = Rect::from_min_size(top, Vec2::new(2.0, LINE_HEIGHT));
                painter.rect_filled(rect, 0.0, cursor_color);
                cursor_rect = Some(rect);
            }
        }

        let highlight = self.core.highlight();
        let renderer = match &mut self.renderer {
            Ok(renderer) => renderer,
            Err(_) => return cursor_rect,
        };
        for line in &layout.lines {
            renderer.paint_line(ui.ctx(), painter, origin, line, |byte| {
                syntax_color(highlight.advance_and_read(byte))
            });
        }
        cursor_rect
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

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        let id = egui::Id::new(("text-editor", self.block.id()));
        let mut reveal_cursor = self.keyboard_input(ui, id);
        let Some(bytes) = self.block.read().map(|document| document.bytes().to_vec()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let layout = match &self.renderer {
            Ok(renderer) => renderer.layout(&bytes),
            Err(error) => {
                ui.centered_and_justified(|ui| {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                });
                return None;
            }
        };

        egui::ScrollArea::both()
            .id_salt(id.with("scroll"))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let desired = layout.size.max(ui.available_size());
                let (rect, _) = ui.allocate_exact_size(desired, Sense::hover());
                let response = ui.interact(rect, id, Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                let response = response.on_hover_cursor(egui::CursorIcon::Text);
                ui.painter()
                    .rect_filled(response.rect, 0.0, Color32::from_rgb(29, 37, 44));
                let origin = response.rect.min + PADDING;
                reveal_cursor |= self.pointer_input(ui, &response, origin, &layout);
                let cursor = self.paint(ui, &painter, origin, &layout);
                if reveal_cursor {
                    if let Some(cursor) = cursor {
                        ui.scroll_to_rect(cursor.expand2(Vec2::new(8.0, 3.0)), None);
                    }
                }
            });
        None
    }
}

fn hit_test(layout: &DocumentLayout, point: Vec2) -> usize {
    let line = ((point.y / LINE_HEIGHT).floor().max(0.0) as usize)
        .min(layout.lines.len().saturating_sub(1));
    let line_layout = &layout.lines[line];
    (line_layout.start..=line_layout.end)
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
        SynHlColorScope::Unstyled => 0xb7c5d3,
        SynHlColorScope::Invisible => 0x43515c,
    }
}
