mod effects;
mod player;
mod timeline;

use std::collections::HashMap;

use block::{BlockParent, BlockReference, BlockReferenceList};
use block_client::{
    block_ref::BlockRef,
    blocks::{
        video::{Video, VideoAttachment, VideoClip, VideoFrameRate, VideoOperation},
        workspace_index::BlockEntry,
    },
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui::{self, Sense, Vec2};
use egui_material_icons::{
    icons::{
        ICON_ADD, ICON_CONTENT_CUT, ICON_DELETE, ICON_FIT_SCREEN, ICON_MOVIE, ICON_PAUSE,
        ICON_PLAY_ARROW, ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_SUBDIRECTORY_ARROW_RIGHT,
        ICON_ZOOM_IN, ICON_ZOOM_OUT,
    },
    MaterialIcon,
};
use uuid::Uuid;

use crate::block_picker::BlockPicker;

use super::{
    reference_cache::{ReferenceClassificationQueue, ReferenceResolutionCache},
    BlockEditor, BlockRenderContext, CreatableEditor, DirectEditorCapabilities, DirectEditorResize,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

use self::timeline::{MAX_PIXELS_PER_FRAME, MIN_PIXELS_PER_FRAME};

const EFFECTS_WIDTH: f32 = 268.0;
const MIN_TIMELINE_HEIGHT: f32 = 176.0;
const MAX_TIMELINE_HEIGHT: f32 = 380.0;
const DEFAULT_EDITOR_SIZE: Vec2 = egui::vec2(1000.0, 640.0);
const DEFAULT_PIXELS_PER_FRAME: f32 = 4.0;
/// How long a clip is when it is first added.
const DEFAULT_CLIP_SECONDS: f64 = 5.0;
const PANEL_GAP: f32 = 6.0;
/// Height of the playback controls strip under the preview panel.
const PLAYBACK_BAR_HEIGHT: f32 = 30.0;
/// Height of the toolbar strip above the timeline panel.
const TIMELINE_TOOLBAR_HEIGHT: f32 = 30.0;

impl EditorKind for VideoEditor {
    type Block = Video;

    const DISPLAY_NAME: &'static str = "Video Editor";
    const ICON: MaterialIcon = ICON_MOVIE;
    const CAN_ADD_CHILD: bool = true;
    const CAN_DELETE_CHILD: bool = true;
    const CAN_REPLACE_CHILD: bool = true;

    fn open(client: &BlockClient, block: BlockHandle<Video>) -> Self {
        Self::new(block, client)
    }
}

impl CreatableEditor for VideoEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(Video::new()), client)
    }
}

/// What a timeline drag is holding onto: the clip, and how far into it the
/// pointer grabbed, so the clip does not jump under the cursor.
pub(super) struct ClipDrag {
    clip: Uuid,
    grab: u64,
}

pub(super) struct VideoEditor {
    block: BlockHandle<Video>,
    dependencies: ReferenceList,
    picker: BlockPicker,
    /// The clip a picked block is attached to, or `None` for the base track.
    picker_attachment: Option<Uuid>,
    selected: Option<Uuid>,
    playhead: u64,
    playing: bool,
    /// The wall clock reading and frame playback started from.
    play_origin: Option<(f64, u64)>,
    /// Timeline zoom, in points per frame.
    pixels_per_frame: f32,
    /// Set by the fit button, which needs the timeline's width to answer.
    fit_requested: bool,
    drag: Option<ClipDrag>,
    reference_cache: ReferenceResolutionCache,
    pending_clips: ReferenceClassificationQueue<(Uuid, u64, Option<VideoAttachment>, usize)>,
}

impl VideoEditor {
    fn new(block: BlockHandle<Video>, client: &BlockClient) -> Self {
        let dependencies = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            dependencies,
            picker: BlockPicker::default(),
            picker_attachment: None,
            selected: None,
            playhead: 0,
            playing: false,
            play_origin: None,
            pixels_per_frame: DEFAULT_PIXELS_PER_FRAME,
            fit_requested: false,
            drag: None,
            reference_cache: ReferenceResolutionCache::default(),
            pending_clips: ReferenceClassificationQueue::default(),
        }
    }

    /// The whole video, copied out so no read guard is held while operating on
    /// the block or drawing the blocks it references.
    fn video(&self) -> Option<Video> {
        self.block.read().map(|video| video.clone())
    }

    fn dependency_map(&self) -> HashMap<Uuid, BlockReference> {
        self.dependencies
            .read()
            .iter()
            .cloned()
            .map(|reference| (reference.id, reference))
            .collect()
    }

    fn poll(&mut self) {
        self.reference_cache.poll();
        for (reference, (clip_id, length, attachment, index)) in self.pending_clips.poll() {
            let clip = VideoClip {
                id: clip_id,
                block_id: reference,
                length,
                attachment,
                effects: Vec::new(),
            };
            self.block
                .operate(VideoOperation::InsertClip { clip, index });
        }
    }

    fn resolve_clips(
        &mut self,
        editors: &EditorAccess<'_>,
        video: &Video,
    ) -> HashMap<BlockRef, Option<Uuid>> {
        let client = editors.client_handle();
        let referencing_id = self.block.id();
        video
            .clips()
            .iter()
            .map(|clip| {
                (
                    clip.block_id,
                    self.reference_cache
                        .resolve(&client, referencing_id, clip.block_id),
                )
            })
            .collect()
    }

    fn ensure_clip_editors(
        video: &Video,
        resolved: &HashMap<BlockRef, Option<Uuid>>,
        dependencies: &HashMap<Uuid, BlockReference>,
        editors: &mut EditorAccess<'_>,
    ) {
        for clip in video.clips() {
            if let Some(reference) = resolved
                .get(&clip.block_id)
                .copied()
                .flatten()
                .and_then(|id| dependencies.get(&id))
            {
                editors.ensure(reference.id, reference.block_type);
            }
        }
    }

    fn synchronize(&mut self, video: &Video) {
        if self
            .selected
            .is_some_and(|selected| video.clip(selected).is_none())
        {
            self.selected = None;
        }
        self.playhead = self.playhead.min(video.duration());
    }

    fn selected_clip<'a>(&self, video: &'a Video) -> Option<&'a VideoClip> {
        video.clip(self.selected?)
    }

    fn update_clip(&self, clip: VideoClip) {
        self.block
            .operate(VideoOperation::UpdateClips { clips: vec![clip] });
    }

    /// Adds a clip showing `block_id`, attached to `attachment` at `frame` or
    /// on the base track. `insertion_index` chooses its place among siblings;
    /// omitting it appends the clip.
    fn insert_clip(
        &mut self,
        editors: &mut EditorAccess<'_>,
        video: &Video,
        block_id: Uuid,
        attachment: Option<Uuid>,
        frame: u64,
        insertion_index: Option<usize>,
    ) {
        let length = video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1);
        let (attachment, index) = match attachment {
            Some(parent) => {
                let parent_start = video.timing(parent).map_or(0, |timing| timing.start);
                let offset = i64::try_from(frame).unwrap_or(i64::MAX)
                    - i64::try_from(parent_start).unwrap_or(0);
                (
                    Some(VideoAttachment::new(parent, offset)),
                    insertion_index.unwrap_or_else(|| video.children(Some(parent)).len()),
                )
            }
            None => (
                None,
                insertion_index.unwrap_or_else(|| video.children(None).len()),
            ),
        };
        let clip_id = Uuid::new_v4();
        self.selected = Some(clip_id);
        let client = editors.client_handle();
        let referencing_id = self.block.id();
        self.pending_clips.push(
            &client,
            referencing_id,
            block_id,
            (clip_id, length, attachment, index),
        );
    }

    /// Splits the selected clip into two at the playhead, keeping the first
    /// half in place and inserting the second half right after it.
    fn split_selected_clip(&mut self, video: &Video) {
        let Some(selected) = self.selected else {
            return;
        };
        let Some(clip) = video.clip(selected) else {
            return;
        };
        let Some(timing) = video.timing(selected) else {
            return;
        };
        if !timing.covers(self.playhead) || self.playhead == timing.start {
            return;
        }
        let first_length = self.playhead - timing.start;
        let second_length = timing.length - first_length;

        let mut first = clip.clone();
        first.length = first_length;

        let mut second = clip.clone();
        second.id = Uuid::new_v4();
        second.length = second_length;
        second.attachment = clip.attachment.map(|attachment| {
            VideoAttachment::new(
                attachment.clip_id,
                attachment.offset + i64::try_from(first_length).unwrap_or(i64::MAX),
            )
        });

        let index = video.sibling_index(selected).unwrap_or(0) + 1;
        self.selected = Some(second.id);
        self.block
            .operate(VideoOperation::UpdateClips { clips: vec![first] });
        self.block.operate(VideoOperation::InsertClip {
            clip: second,
            index,
        });
    }

    fn remove_clip(&mut self, clip_id: Uuid) {
        self.block
            .operate(VideoOperation::RemoveClips { ids: vec![clip_id] });
        if self.selected == Some(clip_id) {
            self.selected = None;
        }
    }

    fn seek(&mut self, frame: u64, duration: u64) {
        self.playhead = frame.min(duration);
        self.play_origin = None;
    }

    /// Runs the playhead off the wall clock. Nothing has audio or video yet, so
    /// playing only moves the playhead over the still previews.
    fn advance_playback(&mut self, context: &egui::Context, video: &Video) {
        let duration = video.duration();
        if !self.playing {
            return;
        }
        if duration == 0 {
            self.playing = false;
            return;
        }
        let now = context.input(|input| input.time);
        let (origin_time, origin_frame) = *self.play_origin.get_or_insert((now, self.playhead));
        self.playhead = origin_frame.saturating_add(video.frame_rate().frames(now - origin_time));
        if self.playhead >= duration {
            self.playhead = duration;
            self.playing = false;
            self.play_origin = None;
        }
        context.request_repaint();
    }

    fn toggle_playback(&mut self, duration: u64) {
        self.playing = !self.playing;
        self.play_origin = None;
        if self.playing && self.playhead >= duration {
            self.playhead = 0;
        }
    }

    fn playback_controls(&mut self, ui: &mut egui::Ui, video: &Video) {
        let duration = video.duration();
        if ui
            .button(ICON_SKIP_PREVIOUS)
            .on_hover_text("Go to the start")
            .clicked()
        {
            self.seek(0, duration);
        }
        let (icon, hover) = if self.playing {
            (ICON_PAUSE, "Pause")
        } else {
            (ICON_PLAY_ARROW, "Play")
        };
        if ui
            .add_enabled(duration > 0, egui::Button::new(icon))
            .on_hover_text(hover)
            .clicked()
        {
            self.toggle_playback(duration);
        }
        if ui
            .button(ICON_SKIP_NEXT)
            .on_hover_text("Go to the end")
            .clicked()
        {
            self.seek(duration, duration);
        }
        ui.label(format!(
            "{} / {}",
            timeline::timecode(video.frame_rate(), self.playhead),
            timeline::timecode(video.frame_rate(), duration)
        ))
        .on_hover_text(format!("Frame {} of {duration}", self.playhead));
    }

    fn clip_menu(&mut self, ui: &mut egui::Ui, _editors: &mut EditorAccess<'_>, video: &Video) {
        if ui
            .button(format!("{} Add clip", ICON_ADD.codepoint))
            .on_hover_text("Add a clip to the end of the base track")
            .clicked()
        {
            self.picker_attachment = None;
            self.picker.open([self.block.id()]);
        }

        let selected = self.selected;
        ui.add_enabled_ui(selected.is_some(), |ui| {
            if ui
                .button(ICON_SUBDIRECTORY_ARROW_RIGHT)
                .on_hover_text(
                    "Attach a clip to the selected clip at the playhead\n\
                     You can also drag any clip onto another clip's row to attach it there.",
                )
                .clicked()
            {
                self.picker_attachment = selected;
                self.picker.open([self.block.id()]);
            }
        });
        if ui
            .add_enabled(selected.is_some(), egui::Button::new(ICON_CONTENT_CUT))
            .on_hover_text("Split the selected clip in two at the playhead")
            .clicked()
        {
            self.split_selected_clip(video);
        }
        if ui
            .add_enabled(selected.is_some(), egui::Button::new(ICON_DELETE))
            .on_hover_text("Delete the selected clip and everything attached to it")
            .clicked()
        {
            if let Some(selected) = selected {
                self.remove_clip(selected);
            }
        }
    }

    fn zoom_controls(&mut self, ui: &mut egui::Ui) {
        if ui
            .button(ICON_ZOOM_OUT)
            .on_hover_text("Zoom the timeline out")
            .clicked()
        {
            self.pixels_per_frame =
                (self.pixels_per_frame / 1.5).clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        }
        if ui
            .button(ICON_ZOOM_IN)
            .on_hover_text("Zoom the timeline in")
            .clicked()
        {
            self.pixels_per_frame =
                (self.pixels_per_frame * 1.5).clamp(MIN_PIXELS_PER_FRAME, MAX_PIXELS_PER_FRAME);
        }
        if ui
            .button(ICON_FIT_SCREEN)
            .on_hover_text("Fit the whole video in the timeline")
            .clicked()
        {
            self.fit_requested = true;
        }
    }

    fn frame_rate_control(&mut self, ui: &mut egui::Ui, video: &Video) {
        let current = video.frame_rate();
        let mut chosen = current;
        egui::ComboBox::from_id_salt(("video-frame-rate", self.block.id()))
            .selected_text(frame_rate_label(current))
            .width(96.0)
            .show_ui(ui, |ui| {
                for rate in FRAME_RATES {
                    ui.selectable_value(&mut chosen, rate, frame_rate_label(rate));
                }
            })
            .response
            .on_hover_text("Frames per second");
        if chosen != current {
            self.block
                .operate(VideoOperation::SetFrameRate { frame_rate: chosen });
        }
    }
}

const FRAME_RATES: [VideoFrameRate; 7] = [
    VideoFrameRate::new(24, 1),
    VideoFrameRate::new(24_000, 1001),
    VideoFrameRate::new(25, 1),
    VideoFrameRate::new(30, 1),
    VideoFrameRate::new(30_000, 1001),
    VideoFrameRate::new(50, 1),
    VideoFrameRate::new(60, 1),
];

fn frame_rate_label(frame_rate: VideoFrameRate) -> String {
    if frame_rate.denominator == 1 {
        return format!("{} fps", frame_rate.numerator);
    }
    format!("{:.2} fps", frame_rate.frames_per_second())
}

impl BlockEditor for VideoEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn add_child(&self, entry: BlockEntry) -> Option<bool> {
        let video = self.block.read()?;
        let length = video.frame_rate().frames(DEFAULT_CLIP_SECONDS).max(1);
        let index = video.children(None).len();
        drop(video);
        self.block.operate(VideoOperation::InsertClip {
            clip: VideoClip::new(BlockRef::Direct(entry.id), length),
            index,
        });
        Some(true)
    }

    fn delete_child(&self, entry: BlockEntry) -> Option<bool> {
        let reference = BlockRef::Direct(entry.id);
        let ids = self
            .block
            .read()?
            .clips()
            .iter()
            .filter(|clip| clip.block_id == reference)
            .map(|clip| clip.id)
            .collect::<Vec<_>>();
        if !ids.is_empty() {
            self.block.operate(VideoOperation::RemoveClips { ids });
        }
        Some(true)
    }

    fn replace_child(&self, old: Uuid, new: BlockEntry) -> Option<bool> {
        let old = BlockRef::Direct(old);
        let new_reference = BlockRef::Direct(new.id);
        let clips = self
            .block
            .read()?
            .clips()
            .iter()
            .filter(|clip| clip.block_id == old)
            .map(|clip| VideoClip {
                block_id: new_reference,
                ..clip.clone()
            })
            .collect::<Vec<_>>();
        if !clips.is_empty() {
            self.block.operate(VideoOperation::UpdateClips { clips });
        }
        Some(true)
    }

    fn render(&mut self, context: BlockRenderContext<'_>, editors: &mut EditorAccess<'_>) -> bool {
        let Some(video) = self.video() else {
            return false;
        };
        let dependencies = self.dependency_map();
        let resolved = self.resolve_clips(editors, &video);
        Self::ensure_clip_editors(&video, &resolved, &dependencies, editors);
        let visible = video.visible_at(self.playhead);
        if visible.is_empty() {
            return false;
        }
        // Clips are listed from the base up, so drawing them in order stacks
        // each attachment over the clip it hangs off.
        for (index, clip_id) in visible.iter().enumerate() {
            let Some(clip) = video.clip(*clip_id) else {
                continue;
            };
            let Some(resolved_id) = resolved.get(&clip.block_id).copied().flatten() else {
                continue;
            };
            let rendered = editors.render(
                resolved_id,
                BlockRenderContext {
                    painter: context.painter,
                    corners: context.corners,
                    opacity: context.opacity,
                },
            );
            if !rendered && index == 0 {
                super::paint_block_fallback(
                    context.painter,
                    egui::Rect::from_min_max(context.corners[0], context.corners[2]),
                    dependencies.get(&resolved_id),
                    editors,
                );
            }
        }
        true
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_resize(&self) -> DirectEditorResize {
        DirectEditorResize::Both
    }

    fn direct_editor_intrinsic_size(&mut self, _editors: &mut EditorAccess<'_>) -> Option<Vec2> {
        Some(DEFAULT_EDITOR_SIZE)
    }

    fn direct_editor_top_bar(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let video = self.video()?;
        self.synchronize(&video);
        ui.horizontal_wrapped(|ui| {
            self.frame_rate_control(ui, &video);
        });
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.poll();
        let Some(video) = self.video() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        self.synchronize(&video);
        let dependencies = self.dependency_map();
        let resolved = self.resolve_clips(editors, &video);
        Self::ensure_clip_editors(&video, &resolved, &dependencies, editors);
        self.advance_playback(ui.ctx(), &video);

        if let Some(result) =
            self.picker
                .handle(ui.ctx(), editors, BlockParent::Uuid(self.block.id()))
        {
            let attachment = self.picker_attachment.take();
            self.insert_clip(editors, &video, result.id, attachment, self.playhead, None);
        }

        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, Sense::hover());
        let timeline_height = (rect.height() * 0.42)
            .clamp(MIN_TIMELINE_HEIGHT, MAX_TIMELINE_HEIGHT)
            .min(rect.height() * 0.75);
        let (top, timeline) = rect.split_top_bottom_at_y(rect.bottom() - timeline_height);
        let (effects, player) =
            top.split_left_right_at_x(top.left() + EFFECTS_WIDTH.min(top.width() * 0.45));
        let (player, playback_bar) =
            player.split_top_bottom_at_y(player.bottom() - PLAYBACK_BAR_HEIGHT);
        let (timeline_toolbar, timeline) =
            timeline.split_top_bottom_at_y(timeline.top() + TIMELINE_TOOLBAR_HEIGHT);

        let stroke = ui.visuals().widgets.noninteractive.bg_stroke;
        ui.painter().vline(effects.right(), top.y_range(), stroke);
        ui.painter()
            .hline(player.x_range(), playback_bar.top(), stroke);
        ui.painter()
            .hline(rect.x_range(), timeline_toolbar.top(), stroke);
        ui.painter()
            .hline(timeline_toolbar.x_range(), timeline.top(), stroke);

        let block_id = self.block.id();
        let region = |ui: &mut egui::Ui, salt: &'static str, rect: egui::Rect| {
            ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((salt, block_id))
                    .max_rect(rect.shrink(PANEL_GAP))
                    .layout(egui::Layout::top_down(egui::Align::Min)),
            )
        };

        let mut effects_ui = region(ui, "video-effects", effects);
        effects_ui.set_clip_rect(effects.intersect(ui.clip_rect()));
        self.effects_ui(&mut effects_ui, &video, &resolved, &dependencies, editors);

        let player_rect = player.shrink(PANEL_GAP);
        let mut player_ui = region(ui, "video-player", player);
        player_ui.set_clip_rect(player.intersect(ui.clip_rect()));
        self.player_ui(
            &mut player_ui,
            player_rect,
            &video,
            &resolved,
            &dependencies,
            editors,
        );

        let mut playback_bar_ui = region(ui, "video-playback-bar", playback_bar);
        playback_bar_ui.set_clip_rect(playback_bar.intersect(ui.clip_rect()));
        playback_bar_ui.horizontal_centered(|ui| self.playback_controls(ui, &video));

        let mut timeline_toolbar_ui = region(ui, "video-timeline-toolbar", timeline_toolbar);
        timeline_toolbar_ui.set_clip_rect(timeline_toolbar.intersect(ui.clip_rect()));
        timeline_toolbar_ui.horizontal(|ui| {
            self.clip_menu(ui, editors, &video);
            ui.separator();
            self.zoom_controls(ui);
        });

        let mut timeline_ui = region(ui, "video-timeline-panel", timeline);
        timeline_ui.set_clip_rect(timeline.intersect(ui.clip_rect()));
        self.timeline_ui(&mut timeline_ui, &video, &resolved, &dependencies, editors);
        None
    }

    fn embedded_direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let rect = ui.available_rect_before_wrap();
        let Some(video) = self.video() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        self.synchronize(&video);
        let dependencies = self.dependency_map();
        let resolved = self.resolve_clips(editors, &video);
        Self::ensure_clip_editors(&video, &resolved, &dependencies, editors);
        self.advance_playback(ui.ctx(), &video);
        self.player_ui(ui, rect, &video, &resolved, &dependencies, editors);
        None
    }
}
