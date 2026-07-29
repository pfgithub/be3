mod browser_tab;
mod infinite_canvas;
mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;

use block::{Block, BlockParent};
use block_client::{
    blocks::{
        infinite_canvas::InfiniteCanvas,
        text::TextDocument,
        web_browser_tab::WebBrowserTab,
        workspace_index::{BlockEntry, WorkspaceIndex},
    },
    BlockClient, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use self::{
    browser_tab::WebBrowserTabEditor, infinite_canvas::InfiniteCanvasEditor, text::TextEditor,
    unsupported::UnsupportedEditor, workspace_index::WorkspaceIndexEditor,
};

pub enum EditorAction {
    OpenBlock {
        id: Uuid,
        block_type: Uuid,
    },
    CreateBlock {
        block_type: Uuid,
        parent: Option<Uuid>,
    },
}

#[derive(Clone)]
pub struct SidebarDragPayload {
    pub reference: block::BlockReference,
    pub source: SidebarDragSource,
    pub is_reference: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarDragSource {
    Root,
    Orphaned,
    Block(Uuid),
}

pub trait BlockEditor {
    fn id(&self) -> Uuid;
    fn block_type(&self) -> Uuid;
    fn name(&self) -> String;
    fn relationships(&self) -> Option<BlockRelationships>;
    fn set_parent(&self, parent: BlockParent);
    fn note_backref(&self, id: Uuid);
    fn can_add_child(&self) -> bool {
        false
    }
    fn add_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn can_delete_child(&self) -> bool {
        false
    }
    fn delete_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn block_created(&mut self, _id: Uuid, _block_type: Uuid, _name: String) {}
    fn update_open_tab(&mut self, _frame: &eframe::Frame) {}
    fn set_tab_active(&mut self, _active: bool) {}
    fn tab_closed(&mut self) {}
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        client: &BlockClient,
        frame: &eframe::Frame,
    ) -> Option<EditorAction>;
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
            InfiniteCanvas::TYPE_ID,
            "Canvas",
            |client| {
                Box::new(InfiniteCanvasEditor::new(
                    client.create_block(InfiniteCanvas::new()),
                    client,
                ))
            },
            |client, id| {
                Box::new(InfiniteCanvasEditor::new(
                    client.get_block::<InfiniteCanvas>(id),
                    client,
                ))
            },
        );
        registry.register(
            TextDocument::TYPE_ID,
            "Text",
            |client| Box::new(TextEditor::new(client.create_block(TextDocument::new()))),
            |client, id| Box::new(TextEditor::new(client.get_block::<TextDocument>(id))),
        );
        registry.register(
            WebBrowserTab::TYPE_ID,
            "Web Browser Tab",
            |client| {
                Box::new(WebBrowserTabEditor::new(
                    client.create_block(WebBrowserTab::new()),
                ))
            },
            |client, id| {
                Box::new(WebBrowserTabEditor::new(
                    client.get_block::<WebBrowserTab>(id),
                ))
            },
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
