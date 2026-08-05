use std::collections::HashMap;

use block::BlockReference;
use block_client::blocks::video::Video;
use eframe::egui::{self, Color32, Rect};
use uuid::Uuid;

use crate::editors::{
    fit_rect, paint_block_fallback, rect_corners, BlockRenderContext, EditorAccess,
};

use super::VideoEditor;

const DEFAULT_ASPECT_RATIO: f32 = 16.0 / 9.0;
const PLAYER_MARGIN: f32 = 8.0;

impl VideoEditor {
    /// The frame under the playhead. No clip carries video or audio yet, so
    /// this is the still preview of every block showing at that frame.
    pub(super) fn player_ui(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        video: &Video,
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

        let ratio = editors
            .preview_aspect_ratio(base.block_id)
            .or_else(|| {
                editors
                    .direct_editor_intrinsic_size(base.block_id)
                    .filter(|size| size.x > 0.0 && size.y > 0.0)
                    .map(|size| size.x / size.y)
            })
            .unwrap_or(DEFAULT_ASPECT_RATIO);
        let frame = fit_rect(rect.shrink(PLAYER_MARGIN), ratio);
        painter.rect_filled(frame, 0.0, Color32::BLACK);

        // The clips come out from the base up, so drawing them in order puts
        // each attachment over the clip it hangs off.
        for (index, clip_id) in visible.iter().enumerate() {
            let Some(clip) = video.clip(*clip_id) else {
                continue;
            };
            let rendered = editors.render(
                clip.block_id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rect_corners(frame),
                    opacity: 1.0,
                },
            );
            if !rendered && index == 0 {
                paint_block_fallback(&painter, frame, dependencies.get(&clip.block_id), editors);
            }
        }
    }
}
