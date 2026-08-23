use std::collections::HashMap;

use block::BlockReference;
use block_client::{block_ref::BlockRef, blocks::video::Video};
use eframe::egui::{self, Color32, Rect};
use uuid::Uuid;

use crate::editors::{
    fit_rect, paint_block_fallback, rect_corners, BlockRenderContext, EditorAccess,
};

use super::VideoEditor;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const PLAYER_MARGIN: f32 = 8.0;

impl VideoEditor {
    pub(super) fn player_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        video: &Video,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, 0.0, Color32::from_gray(18));
        let visible = video.visible_at(self.playhead);
        let Some(base) = visible.first().and_then(|id| video.clip(*id)) else {
            painter.text(
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
            .and_then(|id| {
                editors.preview_aspect_ratio(id).or_else(|| {
                    editors
                        .direct_editor_intrinsic_size(id)
                        .filter(|size| size.x > 0.0 && size.y > 0.0)
                        .map(|size| size.x / size.y)
                })
            })
            .unwrap_or(DEFAULT_ASPECT_RATIO);
        let frame = fit_rect(rect.shrink(PLAYER_MARGIN), ratio);
        painter.rect_filled(frame, 0.0, Color32::BLACK);

        for (index, clip_id) in visible.iter().enumerate() {
            let Some(clip) = video.clip(*clip_id) else {
                continue;
            };
            let Some(id) = resolved.get(&clip.block_id).copied().flatten() else {
                continue;
            };
            let rendered = editors.render(
                id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rect_corners(frame),
                    opacity: 1.0,
                },
            );
            if !rendered && index == 0 {
                paint_block_fallback(&painter, frame, dependencies.get(&id), editors);
            }
        }
    }
}
