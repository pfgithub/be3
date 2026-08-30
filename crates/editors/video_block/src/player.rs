use std::collections::HashMap;

use block::BlockReference;
use block_client::{block_ref::BlockRef, blocks::video::Video};
use block_editor_plugin::block_ui::{BlockCatalog, BlockLabel};
use block_editor_plugin::egui::{self, Color32, Rect};
use block_editor_plugin::EditorHost;
use uuid::Uuid;

use crate::app::VideoApp;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const PLAYER_MARGIN: f32 = 8.0;

impl VideoApp {
    pub(crate) fn player_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        video: &Video,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        dependencies: &HashMap<Uuid, BlockReference>,
        host: &EditorHost,
        types: &BlockCatalog,
    ) {
        ui.painter()
            .with_clip_rect(rect)
            .rect_filled(rect, 0.0, Color32::from_gray(18));
        let visible = video.visible_at(self.playhead);
        let Some(base) = visible.first().and_then(|id| video.clip(*id)) else {
            ui.painter().with_clip_rect(rect).text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "No clip at the playhead",
                egui::FontId::proportional(15.0),
                ui.visuals().weak_text_color(),
            );
            return;
        };

        let base_id = resolved.get(&base.block_id).copied().flatten();
        let ratio = base_id
            .and_then(|id| self.clip_aspect_ratio(id))
            .unwrap_or(DEFAULT_ASPECT_RATIO);
        let frame = crate::app::fit_rect(rect.shrink(PLAYER_MARGIN), ratio);
        ui.painter()
            .with_clip_rect(rect)
            .rect_filled(frame, 0.0, Color32::BLACK);

        let mut painted = false;
        for clip_id in &visible {
            let Some(clip) = video.clip(*clip_id) else {
                continue;
            };
            let Some(id) = resolved.get(&clip.block_id).copied().flatten() else {
                continue;
            };
            let Some(reference) = dependencies.get(&id) else {
                continue;
            };
            let handle = crate::app::place_preview(ui, host, frame, id, reference.block_type);
            if let Some(ratio) = handle.aspect_ratio() {
                self.aspect_ratios.insert(id, ratio);
            }
            painted |= handle.available();
        }
        if !painted {
            let label = dependencies
                .get(&base_id.unwrap_or_default())
                .map(|reference| BlockLabel::for_reference(types, reference));
            ui.painter().with_clip_rect(rect).text(
                frame.center(),
                egui::Align2::CENTER_CENTER,
                label.map_or_else(|| "Loading…".to_owned(), |label| label.name),
                egui::FontId::proportional(13.0),
                ui.visuals().weak_text_color(),
            );
        }
    }

    fn clip_aspect_ratio(&self, id: Uuid) -> Option<f32> {
        self.aspect_ratios.get(&id).copied()
    }
}
