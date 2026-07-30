use std::time::{Duration, Instant};

use block::{Block, BlockParent};
use block_client::{blocks::text::TextDocument, BlockHandle, BlockRelationships};
use eframe::egui;
use uuid::Uuid;

use super::{history::History, BlockEditor, EditorAction};

const TEXT_BURST_DELAY: Duration = Duration::from_millis(750);

pub(super) struct TextEditor {
    block: BlockHandle<TextDocument>,
    history: History<TextBurst>,
    group: Option<TextGroup>,
}

struct TextBurst {
    edits: Vec<TextEditAction>,
}

struct TextEditAction {
    left: Option<Uuid>,
    right: Option<Uuid>,
    fallback_index: usize,
    before: Vec<char>,
    after: Vec<char>,
    removed_ids: Vec<Uuid>,
    visible_ids: Vec<Uuid>,
}

struct TextGroup {
    last_edit: Instant,
    start: usize,
    end: usize,
}

impl TextEditor {
    pub(super) fn new(block: BlockHandle<TextDocument>) -> Self {
        Self {
            block,
            history: History::default(),
            group: None,
        }
    }

    fn apply_burst(block: &BlockHandle<TextDocument>, burst: &mut TextBurst, to_after: bool) {
        if to_after {
            for edit in &mut burst.edits {
                apply_text_action(block, edit, true);
            }
        } else {
            for edit in burst.edits.iter_mut().rev() {
                apply_text_action(block, edit, false);
            }
        }
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

    fn supports_undo(&self) -> bool {
        true
    }

    fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    fn undo(&mut self) {
        self.group = None;
        let block = self.block.clone();
        self.history
            .undo(|burst| Self::apply_burst(&block, burst, false));
    }

    fn redo(&mut self) {
        self.group = None;
        let block = self.block.clone();
        self.history
            .redo(|burst| Self::apply_burst(&block, burst, true));
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _client: &block_client::BlockClient,
        _frame: &eframe::Frame,
    ) -> Option<EditorAction> {
        let Some(document) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let original = document.text();
        drop(document);

        let mut edited = original.clone();
        let response = ui.add_sized(
            ui.available_size(),
            egui::TextEdit::multiline(&mut edited).desired_width(f32::INFINITY),
        );
        if response.changed() {
            if let Some((action, start, end)) = apply_text_edit(&self.block, &original, &edited) {
                let now = Instant::now();
                let joins_range = self.group.as_ref().is_some_and(|group| {
                    now.duration_since(group.last_edit) <= TEXT_BURST_DELAY
                        && start <= group.end.saturating_add(1)
                        && group.start <= end.saturating_add(1)
                });
                let joins_group = joins_range
                    && self.history.last_undo().is_some_and(|burst| {
                        !action.removed_ids.iter().any(|removed| {
                            burst
                                .edits
                                .iter()
                                .any(|edit| edit.visible_ids.contains(removed))
                        })
                    });
                if joins_group {
                    if let Some(burst) = self.history.last_undo_mut() {
                        burst.edits.push(action);
                    } else {
                        self.history.push(
                            TextBurst {
                                edits: vec![action],
                            },
                            1,
                        );
                    }
                } else {
                    let bytes = action.before.len() * size_of::<char>()
                        + action.after.len() * size_of::<char>()
                        + action.removed_ids.len() * size_of::<Uuid>()
                        + action.visible_ids.len() * size_of::<Uuid>();
                    self.history.push(
                        TextBurst {
                            edits: vec![action],
                        },
                        bytes,
                    );
                }
                let (start, end) = if joins_group {
                    self.group.as_ref().map_or((start, end), |group| {
                        (group.start.min(start), group.end.max(end))
                    })
                } else {
                    (start, end)
                };
                self.group = Some(TextGroup {
                    last_edit: now,
                    start,
                    end,
                });
            }
        } else if response.lost_focus() {
            self.group = None;
        }
        None
    }
}

fn apply_text_edit(
    block: &BlockHandle<TextDocument>,
    original: &str,
    edited: &str,
) -> Option<(TextEditAction, usize, usize)> {
    let original: Vec<_> = original.chars().collect();
    let edited: Vec<_> = edited.chars().collect();
    let prefix = original
        .iter()
        .zip(&edited)
        .take_while(|(left, right)| left == right)
        .count();
    let max_suffix = original.len().min(edited.len()) - prefix;
    let suffix = original
        .iter()
        .rev()
        .zip(edited.iter().rev())
        .take(max_suffix)
        .take_while(|(left, right)| left == right)
        .count();
    let original_end = original.len() - suffix;
    let edited_end = edited.len() - suffix;
    if prefix == original_end && prefix == edited_end {
        return None;
    }

    let (left, right, removed_ids) = {
        let document = block.read()?;
        (
            prefix
                .checked_sub(1)
                .and_then(|index| document.item_id(index)),
            document.item_id(original_end),
            (prefix..original_end)
                .filter_map(|index| document.item_id(index))
                .collect::<Vec<_>>(),
        )
    };
    for _ in prefix..original_end {
        let operation = block.read()?.remove_operation(prefix).ok()?;
        block.operate(operation);
    }
    let mut visible_ids = Vec::new();
    for (offset, character) in edited[prefix..edited_end].iter().copied().enumerate() {
        let id = Uuid::new_v4();
        let operation = block
            .read()?
            .insert_operation_with_id(prefix + offset, id, character)
            .ok()?;
        block.operate(operation);
        visible_ids.push(id);
    }
    Some((
        TextEditAction {
            left,
            right,
            fallback_index: prefix,
            before: original[prefix..original_end].to_vec(),
            after: edited[prefix..edited_end].to_vec(),
            removed_ids,
            visible_ids,
        },
        prefix,
        edited_end.max(original_end),
    ))
}

fn apply_text_action(
    block: &BlockHandle<TextDocument>,
    action: &mut TextEditAction,
    to_after: bool,
) {
    for id in std::mem::take(&mut action.visible_ids) {
        let operation = block
            .read()
            .and_then(|document| document.remove_item_operation(id));
        if let Some(operation) = operation {
            block.operate(operation);
        }
    }
    let characters = if to_after {
        &action.after
    } else {
        &action.before
    };
    let mut index = block.read().map_or(0, |document| {
        action
            .right
            .and_then(|id| document.item_index(id))
            .or_else(|| {
                action
                    .left
                    .and_then(|id| document.item_index(id))
                    .map(|index| index + 1)
            })
            .unwrap_or_else(|| action.fallback_index.min(document.len()))
    });
    for character in characters {
        let id = Uuid::new_v4();
        let operation = block.read().and_then(|document| {
            document
                .insert_operation_with_id(index, id, *character)
                .ok()
        });
        if let Some(operation) = operation {
            block.operate(operation);
            action.visible_ids.push(id);
            index += 1;
        }
    }
}
