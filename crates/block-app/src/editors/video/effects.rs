use std::collections::HashMap;

use block::BlockReference;
use block_client::{
    block_ref::BlockRef,
    blocks::video::{Video, VideoAttachment, VideoClip, MAX_CLIP_LENGTH},
};
use eframe::egui;
use egui_material_icons::icons::ICON_LINK_OFF;
use uuid::Uuid;

use crate::editors::{BlockLabel, EditorAccess};

use super::VideoEditor;

impl VideoEditor {
    /// The effects stack of the selected clip, with the clip's own placement
    /// above it. No effects exist yet, so the stack is always empty.
    pub(super) fn effects_ui(
        &mut self,
        ui: &mut egui::Ui,
        video: &Video,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &EditorAccess<'_>,
    ) {
        ui.heading("Effects");
        ui.separator();
        let Some(clip) = self.selected_clip(video).cloned() else {
            ui.weak("Select a clip to see its effects.");
            return;
        };

        ui.horizontal(|ui| {
            let reference = resolved
                .get(&clip.block_id)
                .copied()
                .flatten()
                .and_then(|id| dependencies.get(&id));
            let label = reference.map_or_else(
                || egui::RichText::new("Loading…").into(),
                |reference| {
                    BlockLabel::for_reference(editors.registry(), reference).widget_text(ui.style())
                },
            );
            ui.add(egui::Label::new(label).truncate());
        });
        ui.add_space(4.0);

        egui::Grid::new(("video-clip-placement", clip.id))
            .num_columns(2)
            .spacing([8.0, 6.0])
            .show(ui, |ui| {
                self.placement_ui(ui, video, resolved, dependencies, &clip, editors)
            });

        ui.add_space(10.0);
        ui.strong("Effect stack");
        ui.add_space(2.0);
        if clip.effects.is_empty() {
            ui.weak("No effects yet.");
            return;
        }
        for effect in &clip.effects {
            ui.horizontal(|ui| {
                ui.label(if effect.enabled { "On" } else { "Off" });
                ui.label(&effect.name);
            });
        }
    }

    fn placement_ui(
        &mut self,
        ui: &mut egui::Ui,
        video: &Video,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        dependencies: &HashMap<Uuid, BlockReference>,
        clip: &VideoClip,
        editors: &EditorAccess<'_>,
    ) {
        let frame_rate = video.frame_rate();
        let mut updated = clip.clone();

        ui.label("Length");
        ui.horizontal(|ui| {
            ui.add(
                egui::DragValue::new(&mut updated.length)
                    .speed(1.0)
                    .range(1..=MAX_CLIP_LENGTH),
            )
            .on_hover_text("Frames");
            ui.weak(format!("{:.2}s", frame_rate.seconds(updated.length)));
        });
        ui.end_row();

        ui.label("Starts at");
        let start = video.timing(clip.id).map_or(0, |timing| timing.start);
        ui.weak(super::timeline::timecode(frame_rate, start));
        ui.end_row();

        ui.label("Attached to");
        ui.horizontal(|ui| {
            match clip.attachment {
                None => {
                    ui.weak("Base track");
                }
                Some(attachment) => {
                    let name = video
                        .clip(attachment.clip_id)
                        .and_then(|parent| resolved.get(&parent.block_id).copied().flatten())
                        .and_then(|id| dependencies.get(&id))
                        .map_or_else(
                            || egui::RichText::new("Loading…"),
                            |reference| {
                                BlockLabel::for_reference(editors.registry(), reference).rich_text()
                            },
                        );
                    ui.add(egui::Label::new(name).truncate());
                    if ui
                        .small_button(ICON_LINK_OFF)
                        .on_hover_text("Move this clip onto the base track")
                        .clicked()
                    {
                        updated.attachment = None;
                    }
                }
            };
        });
        ui.end_row();

        if let Some(attachment) = clip.attachment {
            ui.label("Offset");
            let mut offset = attachment.offset;
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut offset).speed(1.0))
                    .on_hover_text("Frames after the clip it hangs off starts");
                ui.weak(format!("{:.2}s", frame_rate.seconds(offset.unsigned_abs())));
            });
            ui.end_row();
            if offset != attachment.offset {
                updated.attachment = Some(VideoAttachment::new(attachment.clip_id, offset));
            }
        }

        if updated != *clip {
            self.update_clip(updated);
        }
    }
}
