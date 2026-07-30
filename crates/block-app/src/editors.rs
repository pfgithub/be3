mod browser_tab;
mod infinite_canvas;
mod pixel_art;
mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;

use block::{Block, BlockParent};
use block_client::{
    blocks::{
        infinite_canvas::InfiniteCanvas,
        pixel_art::PixelArt,
        text::TextDocument,
        web_browser_tab::WebBrowserTab,
        workspace_index::{BlockEntry, WorkspaceIndex},
    },
    BlockClient, BlockRelationships,
};
use eframe::egui;
use uuid::Uuid;

use self::{
    browser_tab::WebBrowserTabEditor, infinite_canvas::InfiniteCanvasEditor,
    pixel_art::PixelArtEditor, text::TextEditor, unsupported::UnsupportedEditor,
    workspace_index::WorkspaceIndexEditor,
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
    SetParent {
        id: Uuid,
        parent: Uuid,
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
    fn add_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn delete_child(&self, _entry: BlockEntry) -> Option<bool> {
        None
    }
    fn block_created(
        &mut self,
        _id: Uuid,
        _block_type: Uuid,
        _author: Uuid,
        _name: String,
    ) -> bool {
        false
    }
    fn update_open_tab(&mut self, _frame: &eframe::Frame) {}
    fn set_tab_active(&mut self, _active: bool) {}
    fn tab_closed(&mut self) {}
    fn history(&self) -> Option<&dyn block_client::BlockHistoryHandle> {
        None
    }
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
    can_add_child: bool,
    can_delete_child: bool,
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
            false,
            false,
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
            PixelArt::TYPE_ID,
            "Pixel Art",
            false,
            false,
            |client| Box::new(PixelArtEditor::new(client.create_block(PixelArt::new()))),
            |client, id| Box::new(PixelArtEditor::new(client.get_block::<PixelArt>(id))),
        );
        registry.register(
            TextDocument::TYPE_ID,
            "Text",
            false,
            false,
            |client| Box::new(TextEditor::new(client.create_block(TextDocument::new()))),
            |client, id| Box::new(TextEditor::new(client.get_block::<TextDocument>(id))),
        );
        registry.register(
            WebBrowserTab::TYPE_ID,
            "Web Browser Tab",
            false,
            false,
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
            true,
            true,
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
        can_add_child: bool,
        can_delete_child: bool,
        create: CreateEditor,
        open: OpenEditor,
    ) {
        self.registrations.insert(
            block_type,
            EditorRegistration {
                display_name,
                create,
                open,
                can_add_child,
                can_delete_child,
            },
        );
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn can_add_child(&self, block_type: Uuid) -> bool {
        self.registrations
            .get(&block_type)
            .is_some_and(|registration| registration.can_add_child)
    }

    pub fn can_delete_child(&self, block_type: Uuid) -> bool {
        self.registrations
            .get(&block_type)
            .is_some_and(|registration| registration.can_delete_child)
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
