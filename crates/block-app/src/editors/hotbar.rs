use std::collections::HashMap;

use block::{Block, BlockParent, BlockReferenceList};
use block_client::{
    blocks::{
        compiled_logic::CompiledLogic,
        hotbar::{Hotbar, HotbarOperation, HotbarSlot},
    },
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_DELETE, ICON_FOLDER, ICON_WIDGETS},
    MaterialIcon,
};
use uuid::Uuid;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 460.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 28.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 80.0;

/// The hotbar the logic editors share. Nothing points at it: it is the hotbar
/// block sitting at the root of the block tree, so every grid in a workspace is
/// built from the same palette however it was created.
pub(super) struct RootHotbar {
    roots: ReferenceList,
    block: Option<BlockHandle<Hotbar>>,
}

impl RootHotbar {
    pub(super) fn new(client: &BlockClient) -> Self {
        Self {
            roots: client.watch_references(BlockReferenceList::Roots),
            block: None,
        }
    }

    /// The hotbar found so far, if any.
    pub(super) fn block(&self) -> Option<&BlockHandle<Hotbar>> {
        self.block.as_ref()
    }

    /// Looks the hotbar up in the root listing, which is only there to be
    /// searched once the workspace has answered.
    pub(super) fn find(&mut self, client: &BlockClient) -> Option<&BlockHandle<Hotbar>> {
        if self.block.is_none() {
            let found = self
                .roots
                .read()
                .into_iter()
                .find(|reference| reference.block_type == Hotbar::TYPE_ID)?;
            self.block = Some(client.get_block::<Hotbar>(found.id));
        }
        self.block.as_ref()
    }

    /// The hotbar to write to, creating it at the root when the tree has none.
    /// Nothing is created before the root listing has loaded, so a workspace
    /// that already has a hotbar is never given a second one.
    pub(super) fn ensure(&mut self, client: &BlockClient) -> Option<&BlockHandle<Hotbar>> {
        if self.find(client).is_none() && self.roots.is_loaded() {
            let hotbar = client.create_block(Hotbar::new());
            hotbar.set_parent(BlockParent::Root);
            self.block = Some(hotbar);
        }
        self.block.as_ref()
    }
}

/// A hotbar is arranged from inside the grid editor that uses it. This shows
/// what is pinned, and lets a component be unpinned or opened without having
/// to find a grid first.
pub(super) struct HotbarEditor {
    block: BlockHandle<Hotbar>,
    /// Handles kept only so pinned components can be named.
    components: HashMap<Uuid, BlockHandle<CompiledLogic>>,
}

impl EditorKind for HotbarEditor {
    type Block = Hotbar;

    const DISPLAY_NAME: &'static str = "Hotbar";
    const ICON: MaterialIcon = ICON_WIDGETS;

    fn open(_client: &BlockClient, block: BlockHandle<Hotbar>) -> Self {
        Self {
            block,
            components: HashMap::new(),
        }
    }
}

impl BlockEditor for HotbarEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let rows = count_slots(self.block.read()?.slots());
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_CHROME_HEIGHT + DIRECT_EDITOR_ROW_HEIGHT * rows as f32,
        ))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(hotbar) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let slots = hotbar.slots().to_vec();
        let pinned = hotbar.components();
        drop(hotbar);

        let client = editors.client();
        self.components.retain(|id, _| pinned.contains(id));
        for compiled in &pinned {
            self.components
                .entry(*compiled)
                .or_insert_with(|| client.get_block::<CompiledLogic>(*compiled));
        }

        if slots.is_empty() {
            ui.weak("Nothing is pinned yet. Compiling a grid pins the component it builds.");
            return None;
        }

        let mut action = None;
        let mut unpin = None;
        self.show_slots(ui, &slots, 0, &mut action, &mut unpin);
        if let Some(compiled) = unpin {
            self.block.operate(HotbarOperation::SetSlots {
                slots: without_component(&slots, compiled),
            });
        }
        action
    }
}

impl HotbarEditor {
    fn show_slots(
        &self,
        ui: &mut egui::Ui,
        slots: &[HotbarSlot],
        depth: usize,
        action: &mut Option<EditorAction>,
        unpin: &mut Option<Uuid>,
    ) {
        for slot in slots {
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 16.0);
                match slot {
                    HotbarSlot::Builtin { tool } => {
                        ui.weak(tool);
                    }
                    HotbarSlot::Locked { name } => {
                        ui.weak(format!("{name} (locked)"));
                    }
                    HotbarSlot::Folder { name, .. } => {
                        ui.label(format!("{} {name}", ICON_FOLDER.codepoint));
                    }
                    HotbarSlot::Component { name, compiled } => {
                        let title = self
                            .components
                            .get(compiled)
                            .and_then(BlockHandle::name)
                            .unwrap_or_else(|| name.clone());
                        if ui.link(title).clicked() {
                            *action = Some(EditorAction::OpenBlock {
                                id: *compiled,
                                block_type: CompiledLogic::TYPE_ID,
                            });
                        }
                        if ui
                            .small_button(ICON_DELETE)
                            .on_hover_text("Unpin")
                            .clicked()
                        {
                            *unpin = Some(*compiled);
                        }
                    }
                }
            });
            if let HotbarSlot::Folder { slots, .. } = slot {
                self.show_slots(ui, slots, depth + 1, action, unpin);
            }
        }
    }
}

fn count_slots(slots: &[HotbarSlot]) -> usize {
    slots
        .iter()
        .map(|slot| match slot {
            HotbarSlot::Folder { slots, .. } => 1 + count_slots(slots),
            _ => 1,
        })
        .sum()
}

/// The same tree with every pin of `compiled` taken out, at any depth.
fn without_component(slots: &[HotbarSlot], compiled: Uuid) -> Vec<HotbarSlot> {
    slots
        .iter()
        .filter(|slot| !matches!(slot, HotbarSlot::Component { compiled: pinned, .. } if *pinned == compiled))
        .map(|slot| match slot {
            HotbarSlot::Folder { name, slots } => HotbarSlot::Folder {
                name: name.clone(),
                slots: without_component(slots, compiled),
            },
            other => other.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
