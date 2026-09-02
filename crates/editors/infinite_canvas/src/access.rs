use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use block::BlockParent;
use block_client::BlockClient;
use block_editor_plugin::{
    block_ui::BlockCatalog, egui, ChildHandle, ChildMode, EditorHost, InteractionMode, ResizeMode,
};
use uuid::Uuid;

pub(crate) type DirectEditorInteraction = InteractionMode;
pub(crate) type DirectEditorResize = ResizeMode;

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct DirectEditorCapabilities {
    pub allow_rotation: bool,
    pub preserve_aspect_ratio: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ChildInfo {
    pub active: bool,
    pub interaction: DirectEditorInteraction,
    pub capabilities: DirectEditorCapabilities,
    pub resize: DirectEditorResize,
    pub intrinsic: Option<egui::Vec2>,
}

#[derive(Default)]
pub(crate) struct Children {
    known: HashMap<Uuid, ChildInfo>,
    types: HashMap<Uuid, Uuid>,
    assigned: HashMap<Uuid, egui::Vec2>,
    widths: HashMap<Uuid, f32>,
    placed: u32,
}

#[derive(Clone)]
pub(crate) struct Access {
    host: EditorHost,
    client: Arc<BlockClient>,
    types: Rc<BlockCatalog>,
    children: Rc<RefCell<Children>>,
}

impl Access {
    pub(crate) fn new(
        host: EditorHost,
        client: Arc<BlockClient>,
        children: Rc<RefCell<Children>>,
    ) -> Self {
        let types = host.block_types();
        Self {
            host,
            client,
            types,
            children,
        }
    }

    pub(crate) fn host(&self) -> &EditorHost {
        &self.host
    }

    pub(crate) fn client(&self) -> &BlockClient {
        &self.client
    }

    pub(crate) fn client_handle(&self) -> Arc<BlockClient> {
        Arc::clone(&self.client)
    }

    pub(crate) fn registry(&self) -> &BlockCatalog {
        &self.types
    }

    fn info(&self, id: Uuid) -> Option<ChildInfo> {
        self.children.borrow().known.get(&id).copied()
    }

    pub(crate) fn direct_editor_interaction(&self, id: Uuid) -> Option<DirectEditorInteraction> {
        self.info(id).map(|info| info.interaction)
    }

    pub(crate) fn direct_editor_capabilities(&self, id: Uuid) -> Option<DirectEditorCapabilities> {
        self.info(id).map(|info| info.capabilities)
    }

    pub(crate) fn direct_editor_resize(&self, id: Uuid) -> Option<DirectEditorResize> {
        self.info(id).map(|info| info.resize)
    }

    pub(crate) fn direct_editor_intrinsic_size(&self, id: Uuid) -> Option<egui::Vec2> {
        self.info(id)?.intrinsic
    }

    pub(crate) fn direct_editor_intrinsic_size_for_width(
        &self,
        id: Uuid,
        width: f32,
    ) -> Option<egui::Vec2> {
        self.children.borrow_mut().widths.insert(id, width);
        let intrinsic = self.info(id)?.intrinsic?;
        Some(egui::vec2(width, intrinsic.y))
    }

    pub(crate) fn default_preserve_aspect_ratio(&self, id: Uuid) -> bool {
        self.info(id)
            .is_some_and(|info| info.capabilities.preserve_aspect_ratio)
    }

    pub(crate) fn set_direct_editor_intrinsic_size(&self, id: Uuid, size: egui::Vec2) {
        self.children.borrow_mut().assigned.insert(id, size);
    }

    pub(crate) fn set_parent(&self, id: Uuid, parent: BlockParent) {
        self.client.set_block_parent(id, parent);
    }

    pub(crate) fn ensure(&self, id: Uuid, block_type: Uuid) {
        self.children.borrow_mut().types.insert(id, block_type);
    }

    pub(crate) fn is_frame_child(&self, id: Uuid) -> bool {
        self.info(id).is_some_and(|info| info.active)
    }

    pub(crate) fn block_type(&self, id: Uuid) -> Option<Uuid> {
        self.children
            .borrow()
            .types
            .get(&id)
            .copied()
            .or_else(|| self.client.cached_block(id).map(|block| block.block_type))
    }

    pub(crate) fn begin_frame(&self) {
        let mut children = self.children.borrow_mut();
        children.placed = 0;
        children.assigned.clear();
        children.widths.clear();
    }

    pub(crate) fn render(
        &self,
        ui: &mut egui::Ui,
        id: Uuid,
        corners: [egui::Pos2; 4],
        opacity: f32,
    ) -> bool {
        let width = corners[0].distance(corners[1]);
        let height = corners[0].distance(corners[3]);
        let center = corners
            .iter()
            .fold(egui::Vec2::ZERO, |sum, corner| sum + corner.to_vec2())
            / corners.len() as f32;
        let axis = corners[1] - corners[0];
        let rotation = axis.y.atan2(axis.x);
        let rect = egui::Rect::from_center_size(
            egui::Pos2::ZERO + center,
            egui::vec2(width.max(1.0), height.max(1.0)),
        );
        self.place(ui, id, rect, rotation, opacity, ChildMode::Preview)
            .is_some_and(|handle| handle.available())
    }

    pub(crate) fn place(
        &self,
        ui: &mut egui::Ui,
        block_id: Uuid,
        rect: egui::Rect,
        rotation: f32,
        opacity: f32,
        mode: ChildMode,
    ) -> Option<ChildHandle> {
        let block_type = self.block_type(block_id)?;
        let (assigned, ordinal) = {
            let mut children = self.children.borrow_mut();
            let ordinal = children.placed;
            children.placed += 1;
            (children.assigned.get(&block_id).copied(), ordinal)
        };
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("canvas-child", block_id, ordinal))
                .max_rect(rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );
        child.set_clip_rect(rect.intersect(ui.clip_rect()));
        let handle = self
            .host
            .child_sized(&mut child, rect.size(), block_id, block_type);
        handle.set_mode(mode);
        handle.set_rotation(rotation);
        handle.set_opacity(opacity);
        if let Some(size) = assigned {
            handle.set_intrinsic_size(size);
        }
        self.children.borrow_mut().known.insert(
            block_id,
            ChildInfo {
                active: handle.active(),
                interaction: handle.interaction(),
                capabilities: DirectEditorCapabilities {
                    allow_rotation: handle.capabilities().rotation,
                    preserve_aspect_ratio: handle.capabilities().preserve_aspect_ratio,
                },
                resize: handle.resize(),
                intrinsic: handle.intrinsic_size(),
            },
        );
        Some(handle)
    }
}
