use std::{collections::HashMap, sync::Arc};

use block::Block;
use block_client::{
    block_ref::BlockRef,
    blocks::{
        compiled_logic::CompiledLogic,
        hotbar::{Hotbar, HotbarOperation, HotbarSlot},
    },
    references::ReferenceResolutionCache,
    BlockClient, BlockHandle,
};
use block_editor_plugin::{egui, EditorHost};
use uuid::Uuid;

/// A hotbar is arranged from inside the grid editor that uses it. This shows
/// what is pinned, and lets a component be unpinned or opened without having
/// to find a grid first.
#[derive(Default)]
pub struct HotbarApp {
    host: EditorHost,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<Hotbar>>,
    /// Handles kept only so pinned components can be named.
    components: HashMap<Uuid, BlockHandle<CompiledLogic>>,
    reference_cache: ReferenceResolutionCache,
}

impl HotbarApp {
    fn layout(&self) -> Option<(Vec<HotbarSlot>, Vec<BlockRef>)> {
        let hotbar = self.block.as_ref()?.read()?;
        Some((hotbar.slots().to_vec(), hotbar.component_refs()))
    }

    fn show_slots(
        &self,
        ui: &mut egui::Ui,
        slots: &[HotbarSlot],
        depth: usize,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
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
                        ui.label(name);
                    }
                    HotbarSlot::Component { name, compiled } => {
                        let resolved_id = resolved.get(compiled).copied().flatten();
                        let title = resolved_id
                            .and_then(|id| self.components.get(&id))
                            .and_then(BlockHandle::name)
                            .unwrap_or_else(|| name.clone());
                        if let Some(id) = resolved_id {
                            if ui.link(title).clicked() {
                                self.host.open_block(id, CompiledLogic::TYPE_ID);
                            }
                        } else {
                            ui.weak(format!("{title} (broken link)"));
                        }
                        if ui.small_button("Unpin").clicked() {
                            *unpin = Some(*compiled);
                        }
                    }
                }
            });
            if let HotbarSlot::Folder { slots, .. } = slot {
                self.show_slots(ui, slots, depth + 1, resolved, unpin);
            }
        }
    }
}

impl block_editor_plugin::App for HotbarApp {
    fn connect(&mut self, host: EditorHost, client: BlockClient, block_id: Uuid) {
        let client = Arc::new(client);
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = host;
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.reference_cache.poll();
        let (Some(client), Some(referencing_id)) = (
            self.client.clone(),
            self.block.as_ref().map(BlockHandle::id),
        ) else {
            ui.spinner();
            return;
        };
        let Some((slots, pinned_refs)) = self.layout() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };

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
                .or_insert_with(|| client.get_block::<CompiledLogic>(id));
        }

        if slots.is_empty() {
            ui.weak("Nothing is pinned yet. Compiling a grid pins the component it builds.");
            return;
        }

        let mut unpin = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            self.show_slots(ui, &slots, 0, &resolved, &mut unpin);
        });
        if let (Some(compiled), Some(block)) = (unpin, self.block.as_ref()) {
            block.operate(HotbarOperation::SetSlots {
                slots: without_component(&slots, compiled),
            });
        }
    }
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
