#[cfg(not(target_os = "android"))]
mod browser_tab;
mod clipboard;
pub(crate) mod image;
mod infinite_canvas;
mod pixel_art;
mod text;
mod unsupported;
mod workspace_index;

use std::collections::HashMap;

use block::BlockParent;
use block_client::{blocks::workspace_index::BlockEntry, BlockClient, BlockRelationships};
use eframe::egui;
use egui_material_icons::MaterialIcon;
use uuid::Uuid;

use self::unsupported::UnsupportedEditor;

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

pub struct BlockRenderContext<'a> {
    pub painter: &'a egui::Painter,
    pub corners: [egui::Pos2; 4],
    pub opacity: f32,
}

pub struct EditorAccess<'a> {
    active: Uuid,
    client: &'a BlockClient,
    registry: &'a EditorRegistry,
    editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
}

impl<'a> EditorAccess<'a> {
    pub fn new(
        active: Uuid,
        client: &'a BlockClient,
        registry: &'a EditorRegistry,
        editors: &'a mut HashMap<Uuid, Box<dyn BlockEditor>>,
    ) -> Self {
        Self {
            active,
            client,
            registry,
            editors,
        }
    }

    pub fn client(&self) -> &BlockClient {
        self.client
    }

    pub fn registry(&self) -> &EditorRegistry {
        self.registry
    }

    pub fn insert(&mut self, editor: Box<dyn BlockEditor>) {
        let id = editor.id();
        assert_ne!(id, self.active, "cannot replace the active editor");
        assert!(
            self.editors.insert(id, editor).is_none(),
            "editor {id} is already open"
        );
    }

    pub fn ensure(&mut self, id: Uuid, block_type: Uuid) {
        if id != self.active && !self.editors.contains_key(&id) {
            self.editors
                .insert(id, self.registry.open(self.client, id, block_type));
        }
    }

    pub fn default_preserve_aspect_ratio(&self, id: Uuid) -> bool {
        self.editors
            .get(&id)
            .is_some_and(|editor| editor.default_preserve_aspect_ratio())
    }

    pub fn render_aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.editors
            .get(&id)
            .and_then(|editor| editor.render_aspect_ratio())
    }

    pub fn render(&mut self, id: Uuid, context: BlockRenderContext<'_>) -> bool {
        let Some(mut editor) = self.editors.remove(&id) else {
            return false;
        };
        let rendered = editor.render(context);
        self.editors.insert(id, editor);
        rendered
    }
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
    fn render(&mut self, _context: BlockRenderContext<'_>) -> bool {
        false
    }
    fn render_aspect_ratio(&self) -> Option<f32> {
        None
    }
    fn default_preserve_aspect_ratio(&self) -> bool {
        false
    }
    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        frame: &eframe::Frame,
    ) -> Option<EditorAction>;
}

type CreateEditor = fn(&BlockClient) -> Box<dyn BlockEditor>;
type OpenEditor = fn(&BlockClient, Uuid) -> Box<dyn BlockEditor>;
type RegenerateDynamicArtifact =
    fn(&BlockClient, Uuid, Uuid, &[u8]) -> Result<Box<dyn DynamicArtifactRegeneration>, String>;

pub trait DynamicArtifactRegeneration {
    fn poll(&mut self) -> Option<Result<(), String>>;
}

struct EditorRegistration {
    block_type: Uuid,
    display_name: &'static str,
    icon: MaterialIcon,
    create: Option<CreateEditor>,
    open: OpenEditor,
    can_add_child: bool,
    can_delete_child: bool,
    regenerate_dynamic_artifact: Option<RegenerateDynamicArtifact>,
}

impl EditorRegistration {
    fn regenerate_dynamic_artifact(
        &self,
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
        let regenerate = self.regenerate_dynamic_artifact.ok_or_else(|| {
            format!(
                "{} blocks do not support dynamic artifact regeneration",
                self.display_name
            )
        })?;
        regenerate(client, target_id, target_type, data)
    }
}

pub struct EditorRegistry {
    registrations: HashMap<Uuid, EditorRegistration>,
    new_block_actions: Vec<(&'static str, Uuid)>,
}

impl EditorRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            registrations: HashMap::new(),
            new_block_actions: Vec::new(),
        };
        registry.register(image::registration());
        registry.register(infinite_canvas::registration());
        registry.register(pixel_art::registration());
        registry.register(text::registration());
        #[cfg(not(target_os = "android"))]
        registry.register(browser_tab::registration());
        registry.register(workspace_index::registration());
        registry
    }

    fn register(&mut self, registration: EditorRegistration) {
        if registration.create.is_some() {
            self.new_block_actions
                .push((registration.display_name, registration.block_type));
        }
        self.registrations
            .insert(registration.block_type, registration);
    }

    pub fn new_block_actions(&self) -> &[(&'static str, Uuid)] {
        &self.new_block_actions
    }

    pub fn display_name(&self, block_type: Uuid) -> Option<&'static str> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.display_name)
    }

    pub fn icon(&self, block_type: Uuid) -> Option<MaterialIcon> {
        self.registrations
            .get(&block_type)
            .map(|registration| registration.icon)
    }

    pub fn icon_label(&self, block_type: Uuid, label: &str) -> String {
        self.icon(block_type).map_or_else(
            || label.to_owned(),
            |icon| format!("{} {label}", icon.codepoint),
        )
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

    pub fn regenerate_dynamic_artifact(
        &self,
        source_type: Uuid,
        client: &BlockClient,
        target_id: Uuid,
        target_type: Uuid,
        data: &[u8],
    ) -> Result<Box<dyn DynamicArtifactRegeneration>, String> {
        let registration = self
            .registrations
            .get(&source_type)
            .ok_or_else(|| format!("unsupported dynamic artifact source type {source_type}"))?;
        registration.regenerate_dynamic_artifact(client, target_id, target_type, data)
    }

    pub fn create(&self, client: &BlockClient, block_type: Uuid) -> Option<Box<dyn BlockEditor>> {
        self.registrations
            .get(&block_type)
            .and_then(|registration| registration.create.map(|create| create(client)))
    }

    pub fn open(&self, client: &BlockClient, id: Uuid, block_type: Uuid) -> Box<dyn BlockEditor> {
        self.registrations.get(&block_type).map_or_else(
            || Box::new(UnsupportedEditor::new(id, block_type)) as Box<dyn BlockEditor>,
            |registration| (registration.open)(client, id),
        )
    }
}
