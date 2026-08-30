use std::collections::HashMap;
use std::sync::Arc;

use block::{BlockParent, BlockReference, BlockReferenceList};
use block_client::block_ref::BlockRef;
use block_client::blocks::video::{
    Video, VideoAttachment, VideoClip, VideoFrameRate, VideoOperation, DEFAULT_CLIP_SECONDS,
};
use block_client::references::{ReferenceClassificationQueue, ReferenceResolutionCache};
use block_client::{BlockClient, BlockHandle, ReferenceList};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui::{self, Sense, Vec2};
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ADD, ICON_CONTENT_CUT, ICON_DELETE, ICON_FIT_SCREEN, ICON_PAUSE, ICON_PLAY_ARROW,
    ICON_SKIP_NEXT, ICON_SKIP_PREVIOUS, ICON_SUBDIRECTORY_ARROW_RIGHT, ICON_ZOOM_IN, ICON_ZOOM_OUT,
};
use block_editor_plugin::{BlockFilter, BlockPicker, ChildHandle, ChildMode, EditorHost};
use uuid::Uuid;

use crate::timeline::{self, MAX_PIXELS_PER_FRAME, MIN_PIXELS_PER_FRAME};

const EFFECTS_WIDTH: f32 = 268.0;
const MIN_TIMELINE_HEIGHT: f32 = 176.0;
const MAX_TIMELINE_HEIGHT: f32 = 380.0;
const DEFAULT_EDITOR_SIZE: Vec2 = egui::vec2(1000.0, 640.0);
const DEFAULT_PIXELS_PER_FRAME: f32 = 4.0;
const PANEL_GAP: f32 = 6.0;
const PLAYBACK_BAR_HEIGHT: f32 = 30.0;
const TIMELINE_TOOLBAR_HEIGHT: f32 = 30.0;

pub(crate) struct ClipDrag {
    pub(crate) clip: Uuid,
    pub(crate) grab: u64,
}

struct Editing {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<Video>,
    dependencies: ReferenceList,
}

#[derive(Default)]
pub struct VideoApp {
    editing: Option<Editing>,
    creation: Option<Arc<BlockClient>>,
    picker: BlockPicker,
    picker_attachment: Option<Uuid>,
    pub(crate) selected: Option<Uuid>,
    pub(crate) playhead: u64,
    playing: bool,
    play_origin: Option<(f64, u64)>,
    pub(crate) pixels_per_frame: f32,
    pub(crate) fit_requested: bool,
    pub(crate) drag: Option<ClipDrag>,
    pub(crate) aspect_ratios: HashMap<Uuid, f32>,
    reference_cache: ReferenceResolutionCache,
    pending_clips: ReferenceClassificationQueue<(Uuid, u64, Option<VideoAttachment>, usize)>,
}

impl VideoApp {
    pub(crate) fn block_id(&self) -> Uuid {
        self.editing
            .as_ref()
            .map_or_else(Uuid::nil, |editing| editing.block.id())
    }

    pub(crate) fn operate(&self, operation: VideoOperation) {
        if let Some(editing) = &self.editing {
            editing.block.operate(operation);
        }
    }

    pub(crate) fn set_child_parent(&self, block_id: Uuid) {
        if let Some(editing) = &self.editing {
            editing
                .client
                .set_block_parent(block_id, BlockParent::Uuid(editing.block.id()));
        }
    }

    fn video(&self) -> Option<Video> {
        self.editing
            .as_ref()?
            .block
            .read()
            .map(|video| video.clone())
    }

    fn dependency_map(&self) -> HashMap<Uuid, BlockReference> {
        self.editing
            .as_ref()
            .map(|editing| editing.dependencies.read())
            .unwrap_or_default()
            .into_iter()
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
            self.operate(VideoOperation::InsertClip { clip, index });
        }
    }

    fn resolve_clips(&mut self, video: &Video) -> HashMap<BlockRef, Option<Uuid>> {
        let Some(client) = self
            .editing
            .as_ref()
            .map(|editing| Arc::clone(&editing.client))
        else {
            return HashMap::new();
        };
        let referencing_id = self.block_id();
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

    fn synchronize(&mut self, video: &Video) {
        if self
            .selected
            .is_some_and(|selected| video.clip(selected).is_none())
        {
            self.selected = None;
        }
        self.playhead = self.playhead.min(video.duration());
    }

    pub(crate) fn selected_clip<'a>(&self, video: &'a Video) -> Option<&'a VideoClip> {
        video.clip(self.selected?)
    }

    pub(crate) fn update_clip(&self, clip: VideoClip) {
        self.operate(VideoOperation::UpdateClips { clips: vec![clip] });
    }

    pub(crate) fn insert_clip(
        &mut self,
        video: &Video,
        block_id: Uuid,
        attachment: Option<Uuid>,
        frame: u64,
        insertion_index: Option<usize>,
    ) {
        let Some(client) = self
            .editing
            .as_ref()
            .map(|editing| Arc::clone(&editing.client))
        else {
            return;
        };
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
        let referencing_id = self.block_id();
        self.pending_clips.push(
            &client,
            referencing_id,
            block_id,
            (clip_id, length, attachment, index),
        );
    }

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
        self.operate(VideoOperation::UpdateClips { clips: vec![first] });
        self.operate(VideoOperation::InsertClip {
            clip: second,
            index,
        });
    }

    fn remove_clip(&mut self, clip_id: Uuid) {
        self.operate(VideoOperation::RemoveClips { ids: vec![clip_id] });
        if self.selected == Some(clip_id) {
            self.selected = None;
        }
    }

    pub(crate) fn seek(&mut self, frame: u64, duration: u64) {
        self.playhead = frame.min(duration);
        self.play_origin = None;
    }

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
        let (icon, hover) = match self.playing {
            true => (ICON_PAUSE, "Pause"),
            false => (ICON_PLAY_ARROW, "Play"),
        };
        if ui
            .add_enabled(duration > 0, egui::Button::new(icon))
            .on_hover_text(hover)
            .test_id("video.play")
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

    fn clip_menu(&mut self, ui: &mut egui::Ui, video: &Video) {
        let Some(host) = self.editing.as_ref().map(|editing| editing.host.clone()) else {
            return;
        };
        if ui
            .button(format!("{} Add clip", ICON_ADD.codepoint))
            .on_hover_text("Add a clip to the end of the base track")
            .test_id("video.add-clip")
            .clicked()
        {
            self.picker_attachment = None;
            self.picker.open(&host, BlockFilter::default());
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
                self.picker.open(&host, BlockFilter::default());
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
        egui::ComboBox::from_id_salt(("video-frame-rate", self.block_id()))
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
            self.operate(VideoOperation::SetFrameRate { frame_rate: chosen });
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

pub(crate) fn fit_rect(available: egui::Rect, ratio: f32) -> egui::Rect {
    let ratio = ratio.max(0.01);
    let available_ratio = available.width() / available.height().max(1.0);
    let size = match available_ratio > ratio {
        true => egui::Vec2::new(available.height() * ratio, available.height()),
        false => egui::Vec2::new(available.width(), available.width() / ratio),
    };
    egui::Rect::from_center_size(available.center(), size)
}

pub(crate) fn place_preview(
    ui: &mut egui::Ui,
    host: &EditorHost,
    rect: egui::Rect,
    block_id: Uuid,
    block_type: Uuid,
) -> ChildHandle {
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(("video-preview", block_id, rect.min.x.to_bits()))
            .max_rect(rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    let handle = host.child_sized(&mut child, rect.size(), block_id, block_type);
    handle.set_mode(ChildMode::Preview);
    handle
}

impl block_editor_plugin::App for VideoApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.pixels_per_frame = DEFAULT_PIXELS_PER_FRAME;
        self.editing = Some(Editing {
            host,
            block: client.get_block(block_id),
            dependencies: client.watch_references(BlockReferenceList::References(block_id)),
            client,
        });
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(Video::new()).id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(DEFAULT_EDITOR_SIZE)
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(video) = self.video() else {
            return;
        };
        self.synchronize(&video);
        ui.horizontal_wrapped(|ui| {
            self.frame_rate_control(ui, &video);
        });
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(types)) = (
            self.editing.as_ref().map(|editing| editing.host.clone()),
            self.editing
                .as_ref()
                .map(|editing| editing.host.block_types()),
        ) else {
            return;
        };
        let Some(video) = self.video() else {
            return;
        };
        self.synchronize(&video);
        let dependencies = self.dependency_map();
        let resolved = self.resolve_clips(&video);
        self.advance_playback(ui.ctx(), &video);
        let rect = ui.max_rect();
        self.player_ui(
            ui,
            rect,
            &video,
            &resolved,
            &dependencies,
            &host,
            types.as_ref(),
        );
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
        let (Some(host), Some(types)) = (
            self.editing.as_ref().map(|editing| editing.host.clone()),
            self.editing
                .as_ref()
                .map(|editing| editing.host.block_types()),
        ) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(video) = self.video() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        self.synchronize(&video);
        let dependencies = self.dependency_map();
        let resolved = self.resolve_clips(&video);
        self.advance_playback(ui.ctx(), &video);

        if let Some(Ok((picked, _))) = self.picker.poll(&host) {
            let attachment = self.picker_attachment.take();
            self.set_child_parent(picked);
            self.insert_clip(&video, picked, attachment, self.playhead, None);
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

        let block_id = self.block_id();
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
        self.effects_ui(
            &mut effects_ui,
            &video,
            &resolved,
            &dependencies,
            types.as_ref(),
        );

        let player_rect = player.shrink(PANEL_GAP);
        let mut player_ui = region(ui, "video-player", player);
        player_ui.set_clip_rect(player.intersect(ui.clip_rect()));
        self.player_ui(
            &mut player_ui,
            player_rect,
            &video,
            &resolved,
            &dependencies,
            &host,
            types.as_ref(),
        );

        let mut playback_bar_ui = region(ui, "video-playback-bar", playback_bar);
        playback_bar_ui.set_clip_rect(playback_bar.intersect(ui.clip_rect()));
        playback_bar_ui.horizontal_centered(|ui| self.playback_controls(ui, &video));

        let mut timeline_toolbar_ui = region(ui, "video-timeline-toolbar", timeline_toolbar);
        timeline_toolbar_ui.set_clip_rect(timeline_toolbar.intersect(ui.clip_rect()));
        timeline_toolbar_ui.horizontal(|ui| {
            self.clip_menu(ui, &video);
            ui.separator();
            self.zoom_controls(ui);
        });

        let mut timeline_ui = region(ui, "video-timeline-panel", timeline);
        timeline_ui.set_clip_rect(timeline.intersect(ui.clip_rect()));
        self.timeline_ui(
            &mut timeline_ui,
            &video,
            &resolved,
            &dependencies,
            &host,
            types.as_ref(),
        );
    }
}
