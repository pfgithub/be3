use std::collections::HashMap;

use block::{BlockParent, BlockReference};
use block_client::blocks::video::{
    Video, VideoAttachment, VideoClip, VideoClipTiming, VideoFrameRate, VideoOperation,
};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Vec2};
use uuid::Uuid;

use crate::editors::{rect_corners, BlockRenderContext, EditorAccess, SidebarDragPayload};

use super::{ClipDrag, VideoEditor};

pub(super) const MIN_PIXELS_PER_FRAME: f32 = 0.02;
pub(super) const MAX_PIXELS_PER_FRAME: f32 = 40.0;

const RULER_HEIGHT: f32 = 22.0;
const LANE_HEIGHT: f32 = 38.0;
const LANE_GAP: f32 = 3.0;
const TRIM_HANDLE_WIDTH: f32 = 6.0;
/// Empty timeline kept past the end so the last clip can be dragged out.
const TAIL_PADDING: f32 = 240.0;
const MIN_TICK_SPACING: f64 = 64.0;
const TICK_SECONDS: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0,
];

/// A clip and the row it is drawn on. Base clips share row zero; every
/// attached clip gets a row of its own, under the clip it hangs off.
struct ClipRow {
    timing: VideoClipTiming,
    lane: usize,
}

/// `frame` as minutes, seconds and frames.
pub(super) fn timecode(frame_rate: VideoFrameRate, frame: u64) -> String {
    let fps = (frame_rate.frames_per_second().round() as u64).max(1);
    let seconds = frame / fps;
    format!("{}:{:02}.{:02}", seconds / 60, seconds % 60, frame % fps)
}

fn lane_rows(video: &Video) -> Vec<ClipRow> {
    let mut next_lane = 1;
    video
        .timeline()
        .into_iter()
        .map(|timing| {
            let lane = if timing.depth == 0 {
                0
            } else {
                next_lane += 1;
                next_lane - 1
            };
            ClipRow { timing, lane }
        })
        .collect()
}

fn lane_rect(content: Rect, lane: usize) -> Rect {
    let top = content.top() + RULER_HEIGHT + lane as f32 * (LANE_HEIGHT + LANE_GAP);
    Rect::from_min_max(
        egui::pos2(content.left(), top),
        egui::pos2(content.right(), top + LANE_HEIGHT),
    )
}

/// The lane a `y` coordinate falls in. Lane zero is the base track; every
/// other lane belongs to whichever clip is attached there.
fn lane_at(content: Rect, y: f32) -> usize {
    ((y - content.top() - RULER_HEIGHT) / (LANE_HEIGHT + LANE_GAP))
        .floor()
        .max(0.0) as usize
}

/// Which clip owns `lane`, so a drag can tell what it would attach to.
fn clip_in_lane(rows: &[ClipRow], lane: usize) -> Option<Uuid> {
    rows.iter()
        .find(|row| row.lane == lane)
        .map(|row| row.timing.id)
}

/// The tick spacing, in seconds, that keeps ruler labels readable.
fn tick_seconds(frame_rate: VideoFrameRate, pixels_per_frame: f32) -> f64 {
    let pixels_per_second = frame_rate.frames_per_second() * f64::from(pixels_per_frame);
    TICK_SECONDS
        .into_iter()
        .find(|step| step * pixels_per_second >= MIN_TICK_SPACING)
        .unwrap_or(3600.0)
}

impl VideoEditor {
    pub(super) fn timeline_ui(
        &mut self,
        ui: &mut egui::Ui,
        video: &Video,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        let rows = lane_rows(video);
        let lanes = rows.iter().map(|row| row.lane + 1).max().unwrap_or(1);
        let duration = video.duration();
        let viewport = ui.available_size();
        if std::mem::take(&mut self.fit_requested) && duration > 0 {
            self.pixels_per_frame = ((viewport.x - TAIL_PADDING * 0.25).max(160.0)
                / duration as f32)
                .clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        }
        let block_id = self.block.id();
        let content = Vec2::new(
            (duration as f32 * self.pixels_per_frame + TAIL_PADDING).max(viewport.x),
            (RULER_HEIGHT + lanes as f32 * (LANE_HEIGHT + LANE_GAP)).max(viewport.y),
        );
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .id_salt(("video-timeline", block_id))
            .show(ui, |ui| {
                let (rect, background) = ui.allocate_exact_size(content, Sense::click_and_drag());
                self.draw_timeline(ui, rect, &background, &rows, video, dependencies, editors);
            });
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_timeline(
        &mut self,
        ui: &mut egui::Ui,
        content: Rect,
        background: &egui::Response,
        rows: &[ClipRow],
        video: &Video,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        let pixels_per_frame = self.pixels_per_frame;
        let frame_at = move |x: f32| ((x - content.left()) / pixels_per_frame).max(0.0) as u64;
        let x_at = move |frame: u64| content.left() + frame as f32 * pixels_per_frame;
        let duration = video.duration();
        let visuals = ui.visuals().clone();
        let painter = ui.painter().clone();

        let lanes = rows.iter().map(|row| row.lane + 1).max().unwrap_or(1);
        for lane in 0..lanes {
            painter.rect_filled(
                lane_rect(content, lane),
                3.0,
                if lane == 0 {
                    visuals.faint_bg_color
                } else {
                    visuals.extreme_bg_color
                },
            );
        }
        self.draw_ruler(&painter, content, video, &visuals);

        // Attachments live on their own rows, so a tether shows what each one
        // hangs off. The tether into the selected clip's own parent is bolded
        // so the parenting is visible at a glance.
        for row in rows {
            let Some(parent) = video.clip(row.timing.id).and_then(|clip| clip.parent()) else {
                continue;
            };
            let Some(parent_lane) = rows.iter().find(|other| other.timing.id == parent) else {
                continue;
            };
            let x = x_at(row.timing.start);
            let stroke = if self.selected == Some(row.timing.id) {
                Stroke::new(2.0_f32, visuals.selection.stroke.color)
            } else {
                Stroke::new(1.0_f32, visuals.widgets.noninteractive.bg_stroke.color)
            };
            painter.line_segment(
                [
                    egui::pos2(x, lane_rect(content, parent_lane.lane).bottom()),
                    egui::pos2(x, lane_rect(content, row.lane).top()),
                ],
                stroke,
            );
        }

        let parent_of_selected = self.selected_clip(video).and_then(VideoClip::parent);
        let mut operations = Vec::new();
        let mut reorder = None;
        for row in rows {
            let Some(clip) = video.clip(row.timing.id).cloned() else {
                continue;
            };
            let lane = lane_rect(content, row.lane);
            let clip_rect = Rect::from_min_max(
                egui::pos2(x_at(row.timing.start), lane.top()),
                egui::pos2(
                    x_at(row.timing.start) + (row.timing.length as f32 * pixels_per_frame).max(3.0),
                    lane.bottom(),
                ),
            );
            let selected = self.selected == Some(clip.id);
            let response = ui.interact(
                clip_rect,
                ui.id().with(("video-clip", clip.id)),
                Sense::click_and_drag(),
            );
            let trim = ui.interact(
                Rect::from_min_max(
                    egui::pos2(clip_rect.right() - TRIM_HANDLE_WIDTH, clip_rect.top()),
                    clip_rect.right_bottom(),
                ),
                ui.id().with(("video-trim", clip.id)),
                Sense::drag(),
            );
            if trim.hovered() || trim.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }

            self.paint_clip(
                &painter,
                clip_rect,
                &clip,
                selected,
                parent_of_selected == Some(clip.id),
                dependencies,
                editors,
                &visuals,
            );

            if response.clicked() {
                self.selected = Some(clip.id);
            }
            if trim.dragged() {
                if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                    let length = frame_at(pointer.x).saturating_sub(row.timing.start).max(1);
                    if length != clip.length {
                        let mut trimmed = clip.clone();
                        trimmed.length = length;
                        operations.push(VideoOperation::UpdateClips {
                            clips: vec![trimmed],
                        });
                    }
                }
                continue;
            }
            if response.drag_started() {
                self.selected = Some(clip.id);
                self.drag = Some(ClipDrag {
                    clip: clip.id,
                    grab: ui
                        .ctx()
                        .pointer_interact_pos()
                        .map_or(0, |pointer| frame_at(pointer.x))
                        .saturating_sub(row.timing.start),
                });
            }
            let drag = self.drag.as_ref().filter(|drag| drag.clip == clip.id);
            // Dragging within the clip's own row only repositions it (an
            // offset for an attachment, or nothing yet for a base clip, which
            // only reorders once the drag ends). Dragging into another row
            // attaches or detaches the clip instead, applied on release so the
            // clip does not flicker between parents while still moving.
            if let (true, Some(drag), Some(pointer)) =
                (response.dragged(), drag, ui.ctx().pointer_interact_pos())
            {
                if clip.attachment.is_some() && lane_at(content, pointer.y) == row.lane {
                    let start = frame_at(pointer.x).saturating_sub(drag.grab);
                    if let Some(update) = reattached(video, &clip, start) {
                        operations.push(VideoOperation::UpdateClips {
                            clips: vec![update],
                        });
                    }
                }
            }
            if let (true, Some(drag), Some(pointer)) = (
                response.drag_stopped(),
                drag,
                ui.ctx().pointer_interact_pos(),
            ) {
                let target_lane = lane_at(content, pointer.y);
                if target_lane == row.lane {
                    if clip.attachment.is_none() {
                        reorder = Some((clip.id, frame_at(pointer.x)));
                    }
                } else if target_lane == 0 {
                    if clip.attachment.is_some() {
                        let mut detached = clip.clone();
                        detached.attachment = None;
                        operations.push(VideoOperation::UpdateClips {
                            clips: vec![detached],
                        });
                        reorder = Some((clip.id, frame_at(pointer.x)));
                    }
                } else if let Some(parent) = clip_in_lane(rows, target_lane) {
                    if parent != clip.id {
                        if let Some(parent_start) = video.timing(parent).map(|timing| timing.start)
                        {
                            let start = frame_at(pointer.x).saturating_sub(drag.grab);
                            let offset = i64::try_from(start).unwrap_or(i64::MAX)
                                - i64::try_from(parent_start).unwrap_or(0);
                            let attachment = Some(VideoAttachment::new(parent, offset));
                            if attachment != clip.attachment {
                                let mut attached = clip.clone();
                                attached.attachment = attachment;
                                operations.push(VideoOperation::UpdateClips {
                                    clips: vec![attached],
                                });
                            }
                        }
                    }
                }
            }
        }
        if let Some(dragged) = background.dnd_hover_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Alias);
            }
        }
        if let Some(dragged) = background.dnd_release_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                let pointer = ui.ctx().pointer_interact_pos();
                let attachment = pointer.and_then(|pointer| {
                    let lane = lane_at(content, pointer.y);
                    (lane != 0).then(|| clip_in_lane(rows, lane)).flatten()
                });
                let frame = pointer.map_or(self.playhead, |pointer| frame_at(pointer.x));
                self.insert_clip(video, dragged.reference.id, attachment, frame);
                editors.set_parent(dragged.reference.id, BlockParent::Uuid(self.block.id()));
            }
        }
        if background.clicked() || background.dragged() {
            if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                self.seek(frame_at(pointer.x), duration);
            }
            if background.clicked() {
                self.selected = None;
            }
        }
        if let Some((clip_id, frame)) = reorder {
            self.drag = None;
            if let Some(index) = base_index_at(video, frame) {
                operations.push(VideoOperation::MoveClip { clip_id, index });
            }
        }
        if ui.input(|input| input.pointer.any_released()) {
            self.drag = None;
        }
        for operation in operations {
            self.block.operate(operation);
        }

        let playhead_x = x_at(self.playhead);
        painter.line_segment(
            [
                egui::pos2(playhead_x, content.top()),
                egui::pos2(playhead_x, content.bottom()),
            ],
            Stroke::new(1.5_f32, visuals.selection.stroke.color),
        );
        painter.rect_filled(
            Rect::from_center_size(
                egui::pos2(playhead_x, content.top() + 5.0),
                Vec2::new(9.0, 10.0),
            ),
            2.0,
            visuals.selection.stroke.color,
        );
    }

    fn draw_ruler(
        &self,
        painter: &egui::Painter,
        content: Rect,
        video: &Video,
        visuals: &egui::Visuals,
    ) {
        let ruler = Rect::from_min_max(
            content.left_top(),
            egui::pos2(content.right(), content.top() + RULER_HEIGHT),
        );
        painter.rect_filled(ruler, 0.0, visuals.extreme_bg_color);
        let frame_rate = video.frame_rate();
        let step = tick_seconds(frame_rate, self.pixels_per_frame);
        // Ticks are placed from the time they mark rather than from a whole
        // number of frames, which would drift at rates like 30000/1001.
        let pixels_per_second =
            (frame_rate.frames_per_second() * f64::from(self.pixels_per_frame)) as f32;
        let pixels_per_step = step as f32 * pixels_per_second;
        let ticks = (content.width() / pixels_per_step).ceil() as u32;
        for tick in 0..=ticks {
            let x = content.left() + tick as f32 * pixels_per_step;
            painter.line_segment(
                [
                    egui::pos2(x, ruler.bottom() - 5.0),
                    egui::pos2(x, ruler.bottom()),
                ],
                Stroke::new(1.0_f32, visuals.widgets.noninteractive.fg_stroke.color),
            );
            let seconds = (f64::from(tick) * step) as u64;
            painter.text(
                egui::pos2(x + 3.0, ruler.top() + 2.0),
                egui::Align2::LEFT_TOP,
                format!("{}:{:02}", seconds / 60, seconds % 60),
                egui::FontId::proportional(10.0),
                visuals.weak_text_color(),
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_clip(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        clip: &VideoClip,
        selected: bool,
        is_parent_of_selected: bool,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
        visuals: &egui::Visuals,
    ) {
        let fill = if clip.attachment.is_some() {
            visuals.widgets.active.weak_bg_fill
        } else {
            visuals.widgets.active.bg_fill
        };
        painter.rect_filled(rect, 4.0, fill);
        let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));

        let thumbnail = Rect::from_min_size(
            rect.left_top() + Vec2::splat(3.0),
            Vec2::new((rect.height() - 6.0) * 1.4, rect.height() - 6.0),
        );
        if rect.width() > thumbnail.width() + 8.0 {
            editors.render(
                clip.block_id,
                BlockRenderContext {
                    painter: &painter,
                    corners: rect_corners(thumbnail),
                    opacity: 1.0,
                },
            );
            let name = dependencies
                .get(&clip.block_id)
                .map_or("Loading…", |reference| reference.name.as_str());
            painter.text(
                egui::pos2(thumbnail.right() + 5.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(11.0),
                visuals.strong_text_color(),
            );
        }
        painter.rect_stroke(
            rect,
            4.0,
            if selected {
                Stroke::new(2.0_f32, visuals.selection.stroke.color)
            } else if is_parent_of_selected {
                Stroke::new(2.0_f32, visuals.selection.stroke.color.gamma_multiply(0.7))
            } else {
                Stroke::new(1.0_f32, Color32::from_black_alpha(90))
            },
            egui::StrokeKind::Inside,
        );
    }
}

/// The clip moved so that it starts at `start`, or `None` when it already
/// does. Attachments are offsets, so the clip it hangs off decides the rest.
fn reattached(
    video: &Video,
    clip: &VideoClip,
    start: u64,
) -> Option<block_client::blocks::video::VideoClip> {
    let attachment = clip.attachment?;
    let parent_start = video.timing(attachment.clip_id)?.start;
    let offset =
        i64::try_from(start).unwrap_or(i64::MAX) - i64::try_from(parent_start).unwrap_or(0);
    (offset != attachment.offset).then(|| {
        let mut moved = clip.clone();
        moved.attachment = Some(VideoAttachment::new(attachment.clip_id, offset));
        moved
    })
}

/// Which base track slot `frame` falls in, so a dragged clip knows where it
/// was dropped.
fn base_index_at(video: &Video, frame: u64) -> Option<usize> {
    let timings = video.timeline();
    let covering = timings
        .iter()
        .filter(|timing| timing.depth == 0)
        .find(|timing| timing.covers(frame))?;
    video.sibling_index(covering.id)
}
