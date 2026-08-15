use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use block::{Block, BlockHistory, HistoryDirection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::block_ref::BlockRef;

const EDIT_BURST_DELAY: Duration = Duration::from_millis(750);

/// The longest a single clip may run, in frames. About nine hours at thirty
/// frames a second, which keeps timeline arithmetic far away from overflowing.
pub const MAX_CLIP_LENGTH: u64 = 1_000_000;

/// The largest numerator or denominator a frame rate may be given.
const MAX_FRAME_RATE_PART: u32 = 1_000_000;

/// How many frames pass each second, as an exact ratio so that broadcast rates
/// such as 30000/1001 stay exact.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoFrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl VideoFrameRate {
    pub const DEFAULT: Self = Self::new(60, 1);

    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn frames_per_second(self) -> f64 {
        f64::from(self.numerator) / f64::from(self.denominator)
    }

    /// How long `frames` frames last, in seconds.
    pub fn seconds(self, frames: u64) -> f64 {
        frames as f64 / self.frames_per_second()
    }

    /// How many whole frames fit into `seconds`.
    pub fn frames(self, seconds: f64) -> u64 {
        if !seconds.is_finite() || seconds <= 0.0 {
            return 0;
        }
        (seconds * self.frames_per_second()) as u64
    }

    fn normalized(self) -> Self {
        Self::new(
            self.numerator.clamp(1, MAX_FRAME_RATE_PART),
            self.denominator.clamp(1, MAX_FRAME_RATE_PART),
        )
    }
}

impl Default for VideoFrameRate {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Where a clip hangs: the clip it is attached to, and how many frames after
/// that clip's start it begins.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoAttachment {
    pub clip_id: Uuid,
    pub offset: i64,
}

impl VideoAttachment {
    pub const fn new(clip_id: Uuid, offset: i64) -> Self {
        Self { clip_id, offset }
    }
}

/// An effect applied to a clip. No effect kinds exist yet; the list is
/// reserved so that clips already carry the stack effects will land in.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoEffect {
    pub id: Uuid,
    pub name: String,
    pub enabled: bool,
}

/// A stretch of the timeline showing one block.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoClip {
    pub id: Uuid,
    /// The block the clip shows.
    pub block_id: BlockRef,
    /// How many frames the clip runs for. At least one.
    pub length: u64,
    /// `None` puts the clip on the base track, where clips run back to back in
    /// list order. Otherwise the clip hangs off another clip, which is what
    /// makes deleting a base clip ripple while leaving attachments alone.
    pub attachment: Option<VideoAttachment>,
    pub effects: Vec<VideoEffect>,
}

impl VideoClip {
    pub fn new(block_id: BlockRef, length: u64) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_id,
            length,
            attachment: None,
            effects: Vec::new(),
        }
    }

    pub fn attached_to(mut self, clip_id: Uuid, offset: i64) -> Self {
        self.attachment = Some(VideoAttachment::new(clip_id, offset));
        self
    }

    /// The clip this one hangs off, if it is not on the base track.
    pub fn parent(&self) -> Option<Uuid> {
        self.attachment.map(|attachment| attachment.clip_id)
    }

    fn offset(&self) -> i64 {
        self.attachment.map_or(0, |attachment| attachment.offset)
    }

    fn normalized(mut self) -> Self {
        self.length = self.length.clamp(1, MAX_CLIP_LENGTH);
        self
    }
}

/// A clip resolved to the place on the timeline its attachments put it.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct VideoClipTiming {
    pub id: Uuid,
    pub start: u64,
    pub length: u64,
    /// How many attachments deep the clip sits. Base track clips are zero.
    pub depth: usize,
}

impl VideoClipTiming {
    pub fn end(&self) -> u64 {
        self.start.saturating_add(self.length)
    }

    pub fn covers(&self, frame: u64) -> bool {
        frame >= self.start && frame < self.end()
    }
}

/// A video built out of clips, each showing another block. One base track runs
/// its clips back to back; every other clip hangs off a clip at a frame offset.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct Video {
    frame_rate: VideoFrameRate,
    clips: Vec<VideoClip>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum VideoOperation {
    /// Adds a clip at `index` among the clips sharing its attachment.
    InsertClip {
        clip: VideoClip,
        index: usize,
    },
    /// Removes clips together with everything attached to them.
    RemoveClips {
        ids: Vec<Uuid>,
    },
    /// Replaces clips by id. An attachment that does not exist, or that would
    /// put a clip inside its own attachments, is left as it was.
    UpdateClips {
        clips: Vec<VideoClip>,
    },
    /// Reorders a clip among the clips sharing its attachment.
    MoveClip {
        clip_id: Uuid,
        index: usize,
    },
    SetFrameRate {
        frame_rate: VideoFrameRate,
    },
}

impl Video {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn frame_rate(&self) -> VideoFrameRate {
        self.frame_rate
    }

    pub fn clips(&self) -> &[VideoClip] {
        &self.clips
    }

    pub fn clip(&self, id: Uuid) -> Option<&VideoClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    /// The clips attached to `parent`, in order. `None` gives the base track.
    pub fn children(&self, parent: Option<Uuid>) -> Vec<&VideoClip> {
        self.clips
            .iter()
            .filter(|clip| clip.parent() == parent)
            .collect()
    }

    /// Where a clip sits among the clips sharing its attachment.
    pub fn sibling_index(&self, clip_id: Uuid) -> Option<usize> {
        let clip = self.clip(clip_id)?;
        self.children(clip.parent())
            .iter()
            .position(|sibling| sibling.id == clip_id)
    }

    /// Every clip resolved to its start frame, each clip before the clips
    /// attached to it. Painting in this order stacks attachments over the clip
    /// they hang off.
    pub fn timeline(&self) -> Vec<VideoClipTiming> {
        let mut timings = Vec::with_capacity(self.clips.len());
        let mut visited = HashSet::new();
        let mut start = 0;
        for clip in self.children(None) {
            self.push_timings(clip, start, 0, &mut visited, &mut timings);
            start = start.saturating_add(clip.length);
        }
        timings
    }

    fn push_timings(
        &self,
        clip: &VideoClip,
        start: u64,
        depth: usize,
        visited: &mut HashSet<Uuid>,
        timings: &mut Vec<VideoClipTiming>,
    ) {
        // Operations never build a cycle, but a stored block that somehow
        // holds one must not be walked forever.
        if !visited.insert(clip.id) {
            return;
        }
        timings.push(VideoClipTiming {
            id: clip.id,
            start,
            length: clip.length,
            depth,
        });
        for child in self.children(Some(clip.id)) {
            let child_start = start.saturating_add_signed(child.offset());
            self.push_timings(child, child_start, depth + 1, visited, timings);
        }
    }

    /// How many frames the whole video runs for.
    pub fn duration(&self) -> u64 {
        self.timeline()
            .iter()
            .map(VideoClipTiming::end)
            .max()
            .unwrap_or(0)
    }

    pub fn timing(&self, clip_id: Uuid) -> Option<VideoClipTiming> {
        self.timeline()
            .into_iter()
            .find(|timing| timing.id == clip_id)
    }

    /// The clips showing at `frame`, bottom first.
    pub fn visible_at(&self, frame: u64) -> Vec<Uuid> {
        self.timeline()
            .iter()
            .filter(|timing| timing.covers(frame))
            .map(|timing| timing.id)
            .collect()
    }

    /// The clips that removing `ids` takes with it - the clips themselves and
    /// everything attached to them - each clip before its attachments.
    fn removal_order(&self, ids: &[Uuid]) -> Vec<Uuid> {
        let mut removed: HashSet<Uuid> = ids.iter().copied().collect();
        let mut order = Vec::new();
        // The timeline lists a clip before its attachments, so a clip's parent
        // has already been decided by the time the clip is looked at.
        for timing in self.timeline() {
            let attached_to_removed = self
                .clip(timing.id)
                .and_then(VideoClip::parent)
                .is_some_and(|parent| removed.contains(&parent));
            if removed.contains(&timing.id) || attached_to_removed {
                removed.insert(timing.id);
                order.push(timing.id);
            }
        }
        order
    }

    /// Where in `clips` a clip attached to `parent` belongs when it is the
    /// `index`th of its siblings.
    fn sibling_position(&self, parent: Option<Uuid>, index: usize) -> usize {
        self.clips
            .iter()
            .enumerate()
            .filter(|(_, clip)| clip.parent() == parent)
            .map(|(position, _)| position)
            .nth(index)
            .unwrap_or(self.clips.len())
    }

    /// Whether attaching `clip_id` to `parent` would put the clip inside its
    /// own attachments.
    fn creates_cycle(&self, clip_id: Uuid, parent: Uuid) -> bool {
        let mut visited = HashSet::new();
        let mut current = Some(parent);
        while let Some(id) = current {
            if id == clip_id || !visited.insert(id) {
                return true;
            }
            current = self.clip(id).and_then(VideoClip::parent);
        }
        false
    }

    /// The attachment a clip may actually be given: one that exists and that
    /// keeps the clip out of its own attachments.
    fn accepted_attachment(
        &self,
        clip_id: Uuid,
        attachment: Option<VideoAttachment>,
    ) -> Option<VideoAttachment> {
        let attachment = attachment?;
        let known = self.clip(attachment.clip_id).is_some();
        (known && !self.creates_cycle(clip_id, attachment.clip_id)).then_some(attachment)
    }
}

impl Block for Video {
    type Operation = VideoOperation;
    type History = VideoHistory;

    const TYPE_ID: Uuid = Uuid::from_u128(0x7669_6465_6f5f_626c_6f63_6b00_0000_0001);

    fn apply_operation(video: &mut Self, operation: &Self::Operation) {
        match operation {
            VideoOperation::InsertClip { clip, index } => {
                if video.clip(clip.id).is_some() {
                    return;
                }
                let mut inserted = clip.clone().normalized();
                // A clip cannot hang off something that is not there, so it
                // lands on the base track instead.
                inserted.attachment = video.accepted_attachment(inserted.id, inserted.attachment);
                let position = video.sibling_position(inserted.parent(), *index);
                video.clips.insert(position, inserted);
            }
            VideoOperation::RemoveClips { ids } => {
                let mut removed: HashSet<Uuid> = video.removal_order(ids).into_iter().collect();
                removed.extend(ids.iter().copied());
                video.clips.retain(|clip| !removed.contains(&clip.id));
            }
            VideoOperation::UpdateClips { clips } => {
                for update in clips {
                    let Some(existing) = video.clip(update.id).map(|clip| clip.attachment) else {
                        continue;
                    };
                    let attachment = match update.attachment {
                        // Detaching onto the base track is always allowed.
                        None => None,
                        // An attachment that is not there, or that would put
                        // the clip inside itself, leaves the clip where it is.
                        attachment => video
                            .accepted_attachment(update.id, attachment)
                            .or(existing),
                    };
                    let mut updated = update.clone().normalized();
                    updated.attachment = attachment;
                    if let Some(clip) = video.clips.iter_mut().find(|clip| clip.id == update.id) {
                        *clip = updated;
                    }
                }
            }
            VideoOperation::MoveClip { clip_id, index } => {
                let Some(current) = video.clips.iter().position(|clip| clip.id == *clip_id) else {
                    return;
                };
                let clip = video.clips.remove(current);
                let position = video.sibling_position(clip.parent(), *index);
                video.clips.insert(position, clip);
            }
            VideoOperation::SetFrameRate { frame_rate } => {
                video.frame_rate = frame_rate.normalized();
            }
        }
    }

    fn references(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.clips
            .iter()
            .filter_map(|clip| clip.block_id.as_direct())
            .filter(|block_id| seen.insert(*block_id))
            .collect()
    }
}

pub struct VideoHistory;

pub struct VideoHistoryAction {
    changes: Vec<VideoHistoryChange>,
    recorded_at: Instant,
}

enum VideoHistoryChange {
    Insert {
        clip: VideoClip,
        index: usize,
    },
    /// The removed clips with the sibling index each one had, listed so that
    /// inserting them back in order restores their attachments.
    Remove {
        clips: Vec<(VideoClip, usize)>,
    },
    Update {
        before: Vec<VideoClip>,
        after: Vec<VideoClip>,
    },
    Move {
        clip_id: Uuid,
        before: usize,
        after: usize,
    },
    FrameRate {
        before: VideoFrameRate,
        after: VideoFrameRate,
    },
}

impl BlockHistory<Video> for VideoHistory {
    type Action = VideoHistoryAction;
    type Snapshot = Video;

    fn snapshot(block: &Video) -> Self::Snapshot {
        block.clone()
    }

    fn action(
        before: Video,
        _after: &Video,
        operations: &[VideoOperation],
    ) -> Option<Self::Action> {
        let mut current = before;
        let mut changes = Vec::new();
        for operation in operations {
            let mut next = current.clone();
            Video::apply_operation(&mut next, operation);
            if let Some(change) = change_for(&current, &next, operation) {
                changes.push(change);
            }
            current = next;
        }
        (!changes.is_empty()).then(|| VideoHistoryAction {
            changes,
            recorded_at: Instant::now(),
        })
    }

    fn action_bytes(action: &Self::Action) -> usize {
        action
            .changes
            .iter()
            .map(|change| match change {
                VideoHistoryChange::Insert { .. } => size_of::<VideoClip>(),
                VideoHistoryChange::Remove { clips } => clips.len() * size_of::<VideoClip>(),
                VideoHistoryChange::Update { before, .. } => {
                    before.len() * size_of::<VideoClip>() * 2
                }
                VideoHistoryChange::Move { .. } => size_of::<Uuid>() + size_of::<usize>() * 2,
                VideoHistoryChange::FrameRate { .. } => size_of::<VideoFrameRate>() * 2,
            })
            .sum()
    }

    fn merge(previous: &mut Self::Action, next: Self::Action) -> Result<(), Self::Action> {
        if next.recorded_at.duration_since(previous.recorded_at) > EDIT_BURST_DELAY {
            return Err(next);
        }
        let (
            [VideoHistoryChange::Update {
                after: previous_after,
                ..
            }],
            [VideoHistoryChange::Update {
                after: next_after, ..
            }],
        ) = (previous.changes.as_mut_slice(), next.changes.as_slice())
        else {
            return Err(next);
        };
        let same_clips = previous_after.len() == next_after.len()
            && previous_after
                .iter()
                .zip(next_after)
                .all(|(previous, next)| previous.id == next.id);
        if !same_clips {
            return Err(next);
        }
        previous_after.clone_from(next_after);
        previous.recorded_at = next.recorded_at;
        Ok(())
    }

    fn operations(
        current: &Video,
        action: &mut Self::Action,
        direction: HistoryDirection,
    ) -> Vec<VideoOperation> {
        let changes: Box<dyn Iterator<Item = &VideoHistoryChange> + '_> =
            if direction == HistoryDirection::Undo {
                Box::new(action.changes.iter().rev())
            } else {
                Box::new(action.changes.iter())
            };
        changes
            .flat_map(|change| operations_for(current, change, direction))
            .collect()
    }
}

/// What one operation changed, ready to be reversed.
fn change_for(
    current: &Video,
    next: &Video,
    operation: &VideoOperation,
) -> Option<VideoHistoryChange> {
    match operation {
        VideoOperation::InsertClip { clip, .. } => {
            if current.clip(clip.id).is_some() {
                return None;
            }
            Some(VideoHistoryChange::Insert {
                clip: next.clip(clip.id)?.clone(),
                index: next.sibling_index(clip.id)?,
            })
        }
        VideoOperation::RemoveClips { ids } => {
            let clips = current
                .removal_order(ids)
                .into_iter()
                .filter_map(|id| Some((current.clip(id)?.clone(), current.sibling_index(id)?)))
                .collect::<Vec<_>>();
            (!clips.is_empty()).then_some(VideoHistoryChange::Remove { clips })
        }
        VideoOperation::UpdateClips { clips } => {
            let mut before = Vec::new();
            let mut after = Vec::new();
            for update in clips {
                let (Some(previous), Some(updated)) =
                    (current.clip(update.id), next.clip(update.id))
                else {
                    continue;
                };
                if previous != updated {
                    before.push(previous.clone());
                    after.push(updated.clone());
                }
            }
            (!after.is_empty()).then_some(VideoHistoryChange::Update { before, after })
        }
        VideoOperation::MoveClip { clip_id, .. } => {
            let before = current.sibling_index(*clip_id)?;
            let after = next.sibling_index(*clip_id)?;
            (before != after).then_some(VideoHistoryChange::Move {
                clip_id: *clip_id,
                before,
                after,
            })
        }
        VideoOperation::SetFrameRate { .. } => {
            (current.frame_rate != next.frame_rate).then_some(VideoHistoryChange::FrameRate {
                before: current.frame_rate,
                after: next.frame_rate,
            })
        }
    }
}

fn operations_for(
    current: &Video,
    change: &VideoHistoryChange,
    direction: HistoryDirection,
) -> Vec<VideoOperation> {
    let to_after = direction == HistoryDirection::Redo;
    match change {
        VideoHistoryChange::Insert { clip, index } => {
            if to_after {
                vec![VideoOperation::InsertClip {
                    clip: clip.clone(),
                    index: *index,
                }]
            } else {
                vec![VideoOperation::RemoveClips { ids: vec![clip.id] }]
            }
        }
        VideoHistoryChange::Remove { clips } => {
            if to_after {
                vec![VideoOperation::RemoveClips {
                    ids: clips.iter().map(|(clip, _)| clip.id).collect(),
                }]
            } else {
                clips
                    .iter()
                    .map(|(clip, index)| VideoOperation::InsertClip {
                        clip: clip.clone(),
                        index: *index,
                    })
                    .collect()
            }
        }
        VideoHistoryChange::Update { before, after } => {
            let (expected, desired) = if to_after {
                (before, after)
            } else {
                (after, before)
            };
            let clips = expected
                .iter()
                .zip(desired)
                .filter_map(|(expected, desired)| {
                    current
                        .clip(expected.id)
                        .map(|clip| rebase_clip(clip, expected, desired))
                })
                .collect::<Vec<_>>();
            (!clips.is_empty())
                .then_some(VideoOperation::UpdateClips { clips })
                .into_iter()
                .collect()
        }
        VideoHistoryChange::Move {
            clip_id,
            before,
            after,
        } => vec![VideoOperation::MoveClip {
            clip_id: *clip_id,
            index: if to_after { *after } else { *before },
        }],
        VideoHistoryChange::FrameRate { before, after } => vec![VideoOperation::SetFrameRate {
            frame_rate: if to_after { *after } else { *before },
        }],
    }
}

/// Keeps the fields a concurrent editor changed while restoring the fields
/// this history action owns.
fn rebase_clip(current: &VideoClip, expected: &VideoClip, desired: &VideoClip) -> VideoClip {
    let mut result = current.clone();
    if result.block_id == expected.block_id {
        result.block_id = desired.block_id;
    }
    if result.length == expected.length {
        result.length = desired.length;
    }
    if result.attachment == expected.attachment {
        result.attachment = desired.attachment;
    }
    if result.effects == expected.effects {
        result.effects.clone_from(&desired.effects);
    }
    result
}

#[cfg(test)]
mod tests;
