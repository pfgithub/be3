use std::collections::HashMap;

use block::{Block, BlockParent};
use block_client::{text::TextDocument, BlockClient, BlockHandle, BlockRelationships};
use eframe::egui;
use uuid::Uuid;

use crate::index::{BlockEntry, WorkspaceIndex, WorkspaceIndexOperation};

pub trait BlockEditor {
    fn id(&self) -> Uuid;
    fn block_type(&self) -> Uuid;
    fn relationships(&self) -> Option<BlockRelationships>;
    fn set_parent(&self, parent: BlockParent);
    fn note_backref(&self, id: Uuid);
    fn add_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn ui(&mut self, ui: &mut egui::Ui);
}

type CreateEditor = fn(&BlockClient) -> Box<dyn BlockEditor>;
type OpenEditor = fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>;

struct EditorRegistration {
    display_name: &'static str,
    create: CreateEditor,
    open: OpenEditor,
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
        };
        registry.register(
            TextDocument::TYPE_ID,
            "Text",
            |client| Box::new(TextEditor::new(client.create_block(TextDocument::new()))),
            |client, id| Box::new(TextEditor::new(client.get_block::<TextDocument>(id))),
        );
        registry.register(
            WorkspaceIndex::TYPE_ID,
            "Folder",
            |client| {
                Box::new(WorkspaceIndexEditor::new(
                    client.create_block(WorkspaceIndex::default()),
                ))
            },
            |client, id| {
                Box::new(WorkspaceIndexEditor::new(
                    client.get_block::<WorkspaceIndex>(id),
                ))
            },
        );
        registry
    }

    fn register(
        &mut self,
        block_type: Uuid,
        display_name: &'static str,
        create: CreateEditor,
        open: OpenEditor,
    ) {
        self.registrations.insert(
            block_type,
            EditorRegistration {
                display_name,
                create,
                open,
            },
        );
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn create(&self, client: &BlockClient, block_type: Uuid) -> Option<Box<dyn BlockEditor>> {
        self.registrations
            .get(&block_type)
            .map(|registration| (registration.create)(client))
    }

    pub fn open(&self, client: &BlockClient, id: Uuid, block_type: Uuid) -> Box<dyn BlockEditor> {
        self.registrations.get(&block_type).map_or_else(
            || Box::new(UnsupportedEditor { id, block_type }) as Box<dyn BlockEditor>,
            |registration| (registration.open)(client, id),
        )
    }
}

struct WorkspaceIndexEditor {
    block: BlockHandle<WorkspaceIndex>,
}

impl WorkspaceIndexEditor {
    fn new(block: BlockHandle<WorkspaceIndex>) -> Self {
        Self { block }
    }
}

impl BlockEditor for WorkspaceIndexEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        WorkspaceIndex::TYPE_ID
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

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let index = self.block.read()?;
        let already_present = index
            .entries()
            .iter()
            .any(|existing| existing.id == entry.id);
        drop(index);
        if !already_present {
            self.block.operate(WorkspaceIndexOperation::Add(entry));
        }
        Some(true)
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };

        if index.entries().is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This folder is empty.");
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for entry in index.entries() {
                ui.label(&entry.title).on_hover_text(entry.id.to_string());
            }
        });
    }
}

struct TextEditor {
    block: BlockHandle<TextDocument>,
}

impl TextEditor {
    fn new(block: BlockHandle<TextDocument>) -> Self {
        Self { block }
    }
}

impl BlockEditor for TextEditor {
    fn id(&self) -> Uuid {
        self.block.id()
    }

    fn block_type(&self) -> Uuid {
        TextDocument::TYPE_ID
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

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(document) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let original = document.text();
        drop(document);

        let mut edited = original.clone();
        let response = ui.add_sized(
            ui.available_size(),
            egui::TextEdit::multiline(&mut edited).desired_width(f32::INFINITY),
        );
        if response.changed() {
            apply_text_edit(&self.block, &original, &edited);
        }
    }
}

fn apply_text_edit(block: &BlockHandle<TextDocument>, original: &str, edited: &str) {
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

    for _ in prefix..(original.len() - suffix) {
        let operation = {
            let Some(document) = block.read() else {
                return;
            };
            document.remove_operation(prefix).ok()
        };
        if let Some(operation) = operation {
            block.operate(operation);
        }
    }

    for (offset, character) in edited[prefix..edited.len() - suffix]
        .iter()
        .copied()
        .enumerate()
    {
        let operation = {
            let Some(document) = block.read() else {
                return;
            };
            document.insert_operation(prefix + offset, character).ok()
        };
        if let Some(operation) = operation {
            block.operate(operation);
        }
    }
}

struct UnsupportedEditor {
    id: Uuid,
    block_type: Uuid,
}

impl BlockEditor for UnsupportedEditor {
    fn id(&self) -> Uuid {
        self.id
    }

    fn block_type(&self) -> Uuid {
        self.block_type
    }

    fn relationships(&self) -> Option<BlockRelationships> {
        None
    }

    fn set_parent(&self, _parent: BlockParent) {}

    fn note_backref(&self, _id: Uuid) {}

    fn ui(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.heading("Unsupported block type");
                ui.label(format!("Block: {}", self.id));
                ui.label(format!("Type: {}", self.block_type));
            });
        });
    }
}
