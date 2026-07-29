use std::collections::HashMap;

use block::Block;
use block_client::{text::TextDocument, BlockClient, BlockHandle};
use eframe::egui;
use uuid::Uuid;

pub trait BlockEditor {
    fn id(&self) -> Uuid;
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
