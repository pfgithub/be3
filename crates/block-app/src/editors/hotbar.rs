use std::collections::HashMap;

use block::Block;
use block_client::{
    block_ref::BlockRef,
    blocks::{
        compiled_logic::CompiledLogic,
        hotbar::{Hotbar, HotbarOperation, HotbarSlot},
    },
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_DELETE, ICON_FOLDER, ICON_WIDGETS},
    MaterialIcon,
};
use uuid::Uuid;

use super::{
    reference_cache::ReferenceResolutionCache, BlockEditor, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 460.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 28.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 80.0;

/// A hotbar is arranged from inside the grid editor that uses it. This shows
/// what is pinned, and lets a component be unpinned or opened without having
/// to find a grid first.
pub(super) struct HotbarEditor {
    block: BlockHandle<Hotbar>,
    /// Handles kept only so pinned components can be named.
    components: HashMap<Uuid, BlockHandle<CompiledLogic>>,
    reference_cache: ReferenceResolutionCache,
}

impl EditorKind for HotbarEditor {
    type Block = Hotbar;

    const DISPLAY_NAME: &'static str = "Hotbar";
    const ICON: MaterialIcon = ICON_WIDGETS;

    fn open(_client: &BlockClient, block: BlockHandle<Hotbar>) -> Self {
        Self {
            block,
            components: HashMap::new(),
            reference_cache: ReferenceResolutionCache::default(),
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
        self.reference_cache.poll();
        let Some(hotbar) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let slots = hotbar.slots().to_vec();
        let pinned_refs = hotbar.component_refs();
        drop(hotbar);

        let referencing_id = self.block.id();
        let client = editors.client_handle();
        let resolved: HashMap<BlockRef, Option<Uuid>> = pinned_refs
            .iter()
            .map(|compiled| {
                (
                    *compiled,
                    self.reference_cache
                        .resolve(&client, referencing_id, *compiled),
                )
            })
            .collect();
        let pinned_ids: Vec<Uuid> = resolved.values().copied().flatten().collect();
        self.components.retain(|id, _| pinned_ids.contains(id));
        for id in pinned_ids {
            self.components
                .entry(id)
                .or_insert_with(|| editors.client().get_block::<CompiledLogic>(id));
        }

        if slots.is_empty() {
            ui.weak("Nothing is pinned yet. Compiling a grid pins the component it builds.");
            return None;
        }

        let mut action = None;
        let mut unpin = None;
        self.show_slots(ui, &slots, 0, &resolved, &mut action, &mut unpin);
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
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        action: &mut Option<EditorAction>,
        unpin: &mut Option<BlockRef>,
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
                        let resolved_id = resolved.get(compiled).copied().flatten();
                        let title = resolved_id
                            .and_then(|id| self.components.get(&id))
                            .and_then(BlockHandle::name)
                            .unwrap_or_else(|| name.clone());
                        if let Some(id) = resolved_id {
                            if ui.link(title).clicked() {
                                *action = Some(EditorAction::OpenBlock {
                                    id,
                                    block_type: CompiledLogic::TYPE_ID,
                                });
                            }
                        } else {
                            ui.weak(format!("{title} (broken link)"));
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
                self.show_slots(ui, slots, depth + 1, resolved, action, unpin);
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
fn without_component(slots: &[HotbarSlot], compiled: BlockRef) -> Vec<HotbarSlot> {
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
