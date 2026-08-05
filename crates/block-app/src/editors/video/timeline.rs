use std::collections::HashMap;

use block::{BlockParent, BlockReference};
use block_client::blocks::video::{
    Video, VideoAttachment, VideoClip, VideoClipTiming, VideoFrameRate, VideoOperation,
};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, Vec2};
use uuid::Uuid;

use crate::editors::{rect_corners, BlockRenderContext, EditorAccess, SidebarDragPayload};

use super::{ClipDrag, VideoEditor, DEFAULT_CLIP_SECONDS};

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

#[derive(Clone, Copy)]
enum ClipDropZone {
    Before,
    Center(Rect),
    After,
}

#[derive(Clone, Copy)]
enum TimelineDropTarget {
    Attach {
        parent: Uuid,
        start: u64,
        lane: usize,
        length: u64,
        highlight: Rect,
    },
    Base {
        index: usize,
        x: f32,
    },
    Offset {
        start: u64,
        lane: usize,
        length: u64,
    },
}

fn clip_rect(content: Rect, row: &ClipRow, pixels_per_frame: f32) -> Rect {
    let lane = lane_rect(content, row.lane);
    let left = content.left() + row.timing.start as f32 * pixels_per_frame;
    Rect::from_min_max(
        egui::pos2(left, lane.top()),
        egui::pos2(
            left + (row.timing.length as f32 * pixels_per_frame).max(3.0),
            lane.bottom(),
        ),
    )
}

fn drop_zone(rect: Rect, x: f32) -> ClipDropZone {
    const EDGE_WIDTH: f32 = 10.0;
    if rect.width() <= EDGE_WIDTH * 2.0 {
        return if x < rect.center().x {
            ClipDropZone::Before
        } else {
            ClipDropZone::After
        };
    }
    if x < rect.left() + EDGE_WIDTH {
        ClipDropZone::Before
    } else if x > rect.right() - EDGE_WIDTH {
        ClipDropZone::After
    } else {
        ClipDropZone::Center(Rect::from_min_max(
            egui::pos2(rect.left() + EDGE_WIDTH, rect.top()),
            egui::pos2(rect.right() - EDGE_WIDTH, rect.bottom()),
        ))
    }
}

fn would_create_cycle(video: &Video, clip_id: Uuid, parent: Uuid) -> bool {
    let mut current = Some(parent);
    while let Some(id) = current {
        if id == clip_id {
            return true;
        }
        current = video.clip(id).and_then(VideoClip::parent);
    }
    false
}

/// Converts an insertion boundary in the current base track into the index
/// expected after `moving` has been removed from that track.
fn adjusted_base_index(video: &Video, boundary: usize, moving: Option<Uuid>) -> usize {
    let moving_index = moving.and_then(|clip_id| {
        video
            .clip(clip_id)
            .filter(|clip| clip.attachment.is_none())
            .and_then(|_| video.sibling_index(clip_id))
    });
    boundary.saturating_sub(usize::from(
        moving_index.is_some_and(|moving_index| moving_index < boundary),
    ))
}

fn base_target(
    video: &Video,
    rows: &[ClipRow],
    content: Rect,
    pixels_per_frame: f32,
    pointer_x: f32,
    moving: Option<Uuid>,
) -> TimelineDropTarget {
    let mut boundaries = Vec::new();
    for row in rows.iter().filter(|row| row.timing.depth == 0) {
        if moving == Some(row.timing.id) {
            continue;
        }
        let rect = clip_rect(content, row, pixels_per_frame);
        let index = video.sibling_index(row.timing.id).unwrap_or(0);
        boundaries.push((rect.left(), index));
        boundaries.push((rect.right(), index + 1));
    }
    let (x, boundary) = boundaries
        .into_iter()
        .min_by(|left, right| {
            (left.0 - pointer_x)
                .abs()
                .total_cmp(&(right.0 - pointer_x).abs())
        })
        .unwrap_or((content.left(), 0));
    TimelineDropTarget::Base {
        index: adjusted_base_index(video, boundary, moving),
        x,
    }
}

#[allow(clippy::too_many_arguments)]
fn drop_target_at(
    video: &Video,
    rows: &[ClipRow],
    content: Rect,
    pixels_per_frame: f32,
    pointer: egui::Pos2,
    moving: Option<&ClipDrag>,
) -> Option<TimelineDropTarget> {
    let moving_id = moving.map(|drag| drag.clip);
    let pointer_frame = ((pointer.x - content.left()) / pixels_per_frame).max(0.0) as u64;
    let moved_start = pointer_frame.saturating_sub(moving.map_or(0, |drag| drag.grab));
    let dragged_length = moving.and_then(|drag| video.clip(drag.clip)).map_or_else(
        || video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1),
        |clip| clip.length,
    );

    if let Some((row, rect)) = rows
        .iter()
        .rev()
        .filter(|row| moving_id != Some(row.timing.id))
        .map(|row| (row, clip_rect(content, row, pixels_per_frame)))
        .find(|(_, rect)| rect.contains(pointer))
    {
        match drop_zone(rect, pointer.x) {
            ClipDropZone::Center(highlight) => {
                if moving_id
                    .is_none_or(|clip_id| !would_create_cycle(video, clip_id, row.timing.id))
                {
                    return Some(TimelineDropTarget::Attach {
                        parent: row.timing.id,
                        start: pointer_frame,
                        lane: row.lane + 1,
                        length: dragged_length,
                        highlight,
                    });
                }
            }
            ClipDropZone::Before if row.timing.depth == 0 => {
                let boundary = video.sibling_index(row.timing.id).unwrap_or(0);
                return Some(TimelineDropTarget::Base {
                    index: adjusted_base_index(video, boundary, moving_id),
                    x: rect.left(),
                });
            }
            ClipDropZone::After if row.timing.depth == 0 => {
                let boundary = video.sibling_index(row.timing.id).unwrap_or(0) + 1;
                return Some(TimelineDropTarget::Base {
                    index: adjusted_base_index(video, boundary, moving_id),
                    x: rect.right(),
                });
            }
            ClipDropZone::Before | ClipDropZone::After => {}
        }
    }

    let lane = lane_at(content, pointer.y);
    if let Some(drag) = moving {
        let row = rows.iter().find(|row| row.timing.id == drag.clip)?;
        let clip = video.clip(drag.clip)?;
        if lane == row.lane && clip.attachment.is_some() {
            return Some(TimelineDropTarget::Offset {
                start: moved_start,
                lane,
                length: clip.length,
            });
        }
    }
    (lane == 0).then(|| base_target(video, rows, content, pixels_per_frame, pointer.x, moving_id))
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
        let lanes = rows.iter().map(|row| row.lane + 1).max().unwrap_or(1) + 1;
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

        let lanes = rows.iter().map(|row| row.lane + 1).max().unwrap_or(1) + 1;
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
        for row in rows {
            let Some(clip) = video.clip(row.timing.id).cloned() else {
                continue;
            };
            let clip_rect = clip_rect(content, row, pixels_per_frame);
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
        }

        let pointer = ui
            .ctx()
            .pointer_interact_pos()
            .or_else(|| ui.ctx().pointer_hover_pos());
        let internal_target = self.drag.as_ref().and_then(|drag| {
            pointer.and_then(|pointer| {
                drop_target_at(video, rows, content, pixels_per_frame, pointer, Some(drag))
            })
        });
        let sidebar_payload = background
            .dnd_hover_payload::<SidebarDragPayload>()
            .filter(|dragged| dragged.reference.id != self.block.id());
        let sidebar_target = sidebar_payload.as_ref().and_then(|_| {
            pointer.and_then(|pointer| {
                drop_target_at(video, rows, content, pixels_per_frame, pointer, None)
            })
        });
        if internal_target.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        } else if sidebar_target.is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Alias);
        }
        if let Some(target) = internal_target.or(sidebar_target) {
            paint_drop_target(&painter, content, pixels_per_frame, target, &visuals);
        }

        let released = ui.input(|input| input.pointer.any_released());
        let had_internal_drag = self.drag.is_some();
        if released {
            if let (Some(drag), Some(target)) = (self.drag.as_ref(), internal_target) {
                apply_clip_drop(video, drag.clip, target, &mut operations);
            }
            self.drag = None;
        }
        if let Some(dragged) = background.dnd_release_payload::<SidebarDragPayload>() {
            if dragged.reference.id != self.block.id() {
                if let Some(target) = sidebar_target {
                    match target {
                        TimelineDropTarget::Attach { parent, start, .. } => {
                            self.insert_clip(
                                video,
                                dragged.reference.id,
                                Some(parent),
                                start,
                                Some(0),
                            );
                        }
                        TimelineDropTarget::Base { index, .. } => {
                            self.insert_clip(video, dragged.reference.id, None, 0, Some(index));
                        }
                        TimelineDropTarget::Offset { .. } => {}
                    }
                    editors.set_parent(dragged.reference.id, BlockParent::Uuid(self.block.id()));
                }
            }
        }
        if background.clicked() || (background.dragged() && !had_internal_drag) {
            if let Some(pointer) = ui.ctx().pointer_interact_pos() {
                self.seek(frame_at(pointer.x), duration);
            }
            if background.clicked() {
                self.selected = None;
            }
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

fn paint_drop_target(
    painter: &egui::Painter,
    content: Rect,
    pixels_per_frame: f32,
    target: TimelineDropTarget,
    visuals: &egui::Visuals,
) {
    let color = visuals.selection.stroke.color;
    match target {
        TimelineDropTarget::Attach {
            start,
            lane,
            length,
            highlight,
            ..
        } => {
            painter.rect_filled(highlight.shrink(2.0), 3.0, color.gamma_multiply(0.25));
            painter.rect_stroke(
                highlight.shrink(1.0),
                3.0,
                Stroke::new(2.0_f32, color),
                egui::StrokeKind::Inside,
            );
            let lane = lane_rect(content, lane);
            let left = content.left() + start as f32 * pixels_per_frame;
            let preview = Rect::from_min_max(
                egui::pos2(left, lane.top()),
                egui::pos2(
                    left + (length as f32 * pixels_per_frame).max(3.0),
                    lane.bottom(),
                ),
            );
            painter.rect_filled(preview, 4.0, color.gamma_multiply(0.18));
            painter.rect_stroke(
                preview,
                4.0,
                Stroke::new(2.0_f32, color),
                egui::StrokeKind::Inside,
            );
        }
        TimelineDropTarget::Base { x, .. } => {
            let lane = lane_rect(content, 0);
            painter.line_segment(
                [egui::pos2(x, lane.top()), egui::pos2(x, lane.bottom())],
                Stroke::new(3.0_f32, color),
            );
            painter.circle_filled(egui::pos2(x, lane.top() + 3.0), 4.0, color);
            painter.circle_filled(egui::pos2(x, lane.bottom() - 3.0), 4.0, color);
        }
        TimelineDropTarget::Offset {
            start,
            lane,
            length,
        } => {
            let lane = lane_rect(content, lane);
            let left = content.left() + start as f32 * pixels_per_frame;
            let rect = Rect::from_min_max(
                egui::pos2(left, lane.top()),
                egui::pos2(
                    left + (length as f32 * pixels_per_frame).max(3.0),
                    lane.bottom(),
                ),
            );
            painter.rect_filled(rect, 4.0, color.gamma_multiply(0.18));
            painter.rect_stroke(
                rect,
                4.0,
                Stroke::new(2.0_f32, color),
                egui::StrokeKind::Inside,
            );
        }
    }
}

fn apply_clip_drop(
    video: &Video,
    clip_id: Uuid,
    target: TimelineDropTarget,
    operations: &mut Vec<VideoOperation>,
) {
    let Some(clip) = video.clip(clip_id) else {
        return;
    };
    match target {
        TimelineDropTarget::Attach { parent, start, .. } => {
            let Some(parent_start) = video.timing(parent).map(|timing| timing.start) else {
                return;
            };
            let offset =
                i64::try_from(start).unwrap_or(i64::MAX) - i64::try_from(parent_start).unwrap_or(0);
            let attachment = Some(VideoAttachment::new(parent, offset));
            if clip.attachment != attachment {
                let mut attached = clip.clone();
                attached.attachment = attachment;
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![attached],
                });
            }
            operations.push(VideoOperation::MoveClip { clip_id, index: 0 });
        }
        TimelineDropTarget::Base { index, .. } => {
            if clip.attachment.is_some() {
                let mut detached = clip.clone();
                detached.attachment = None;
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![detached],
                });
            }
            operations.push(VideoOperation::MoveClip { clip_id, index });
        }
        TimelineDropTarget::Offset { start, .. } => {
            if let Some(update) = reattached(video, clip, start) {
                operations.push(VideoOperation::UpdateClips {
                    clips: vec![update],
                });
            }
        }
    }
}
