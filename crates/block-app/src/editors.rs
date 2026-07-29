mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;

use block::{Block, BlockParent};
use block_client::{
    blocks::{
        text::TextDocument,
        workspace_index::{BlockEntry, WorkspaceIndex},
    },
    BlockClient, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use self::{
    text::TextEditor, unsupported::UnsupportedEditor, workspace_index::WorkspaceIndexEditor,
};

pub trait BlockEditor {
    fn id(&self) -> Uuid;
    fn block_type(&self) -> Uuid;
    fn name(&self) -> String;
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
            || Box::new(UnsupportedEditor::new(id, block_type)) as Box<dyn BlockEditor>,
            |registration| (registration.open)(client, id),
        )
    }
}
