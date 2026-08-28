use std::sync::Arc;
use std::time::Duration;

use block::BlockParent;
use block_client::{
    block_ref::BlockRef,
    blocks::{
        paint_review::{ApprovedPainting, PaintReview, PaintReviewOperation},
        paint_snapshot::{PaintSnapshot, PaintSnapshotOperation},
    },
    BlockClient, BlockHandle,
};
use block_editor_plugin::{
    block_ui::test_id::TestId,
    egui,
    egui_material_icons::icons::{
        ICON_CHECK, ICON_CHEVRON_LEFT, ICON_CHEVRON_RIGHT, ICON_COMPARE, ICON_DELETE,
        ICON_DIFFERENCE, ICON_DONE_ALL, ICON_FIBER_NEW, ICON_FIT_SCREEN, ICON_PAUSE,
        ICON_PHOTO_SIZE_SELECT_ACTUAL, ICON_PLAY_ARROW, ICON_REFRESH, ICON_VERTICAL_SPLIT,
        ICON_ZOOM_IN, ICON_ZOOM_OUT,
    },
    EditorHost, Waker,
};
use uuid::Uuid;

use crate::download::{Download, Painting, Source, BRANCH};
use crate::render::{Change, Paintings, Rendered};
use crate::view::{Panel, Viewport};

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(960.0, 640.0);
const FRAME_SECONDS: f64 = 0.25;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Showing {
    Approved,
    #[default]
    Current,
    Difference,
    SideBySide,
}

impl Showing {
    fn label(self) -> String {
        match self {
            Self::Approved => "the painting you approved".to_owned(),
            Self::Current => format!("the painting on the {BRANCH} branch"),
            Self::Difference => "the pixels that changed".to_owned(),
            Self::SideBySide => "approved beside current".to_owned(),
        }
    }

    fn short(self) -> String {
        match self {
            Self::Approved => "Approved".to_owned(),
            _ => format!("On {BRANCH}"),
        }
    }

    fn wanted(self) -> [Self; 2] {
        match self {
            Self::Approved => [Self::Approved, Self::Current],
            _ => [Self::Current, Self::Approved],
        }
    }

    fn both(self) -> bool {
        matches!(self, Self::Difference | Self::SideBySide)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    New,
    Modified,
    Removed,
    Unchanged,
}

impl Status {
    const ALL: [Self; 4] = [Self::New, Self::Modified, Self::Removed, Self::Unchanged];

    fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::Modified => "Modified",
            Self::Removed => "Removed",
            Self::Unchanged => "Approved",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            Self::New => ICON_FIBER_NEW.codepoint,
            Self::Modified => ICON_DIFFERENCE.codepoint,
            Self::Removed => ICON_DELETE.codepoint,
            Self::Unchanged => ICON_DONE_ALL.codepoint,
        }
    }
}

pub struct Entry {
    pub path: String,
    pub status: Status,
}

enum Shown {
    Ready(Vec<Panel>, String),
    Failed(String),
    Waiting(Option<String>),
}

struct Editing {
    host: EditorHost,
    client: Arc<BlockClient>,
    block: BlockHandle<PaintReview>,
}

#[derive(Default)]
pub struct PaintReviewApp {
    editing: Option<Editing>,
    creation: Option<Arc<BlockClient>>,
    source: Source,
    download: Option<Download>,
    found: Vec<Painting>,
    downloaded: bool,
    error: Option<String>,
    selected: Option<String>,
    showing: Showing,
    paintings: Paintings,
    view: Viewport,
    change: Option<(String, Change)>,
    pending: Option<String>,
    frame: usize,
    playing: bool,
    advanced: Option<f64>,
}

impl PaintReviewApp {
    #[cfg(test)]
    pub fn review(&mut self, source: Source) {
        self.source = source;
        self.refresh();
    }

    #[cfg(test)]
    pub fn rasters(&self) -> usize {
        self.paintings.rasters()
    }

    #[cfg(test)]
    pub fn frame(&self) -> usize {
        self.frame
    }

    #[cfg(test)]
    pub fn zoom(&self) -> f32 {
        self.view.scale()
    }

    pub fn entries(&self) -> Option<Vec<Entry>> {
        let approvals = self.approvals()?;
        let mut entries: Vec<Entry> = self
            .found
            .iter()
            .map(|painting| Entry {
                path: painting.path.clone(),
                status: match approvals
                    .iter()
                    .find(|approved| approved.path == painting.path)
                {
                    None => Status::New,
                    Some(approved) if approved.hash != painting.hash => Status::Modified,
                    Some(_) => Status::Unchanged,
                },
            })
            .collect();
        entries.extend(
            approvals
                .iter()
                .filter(|approved| !self.found.iter().any(|found| found.path == approved.path))
                .map(|approved| Entry {
                    path: approved.path.clone(),
                    status: Status::Removed,
                }),
        );
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        Some(entries)
    }

    fn approvals(&self) -> Option<Vec<ApprovedPainting>> {
        Some(self.editing.as_ref()?.block.read()?.approved().to_vec())
    }

    fn approval(&self, path: &str) -> Option<ApprovedPainting> {
        self.approvals()?
            .into_iter()
            .find(|approved| approved.path == path)
    }

    fn status(&self, entries: &[Entry], path: &str) -> Option<Status> {
        entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.status)
    }

    fn showing(&self, status: Status) -> Showing {
        match status {
            Status::Removed => Showing::Approved,
            Status::New | Status::Unchanged => Showing::Current,
            Status::Modified => self.showing,
        }
    }

    fn poll(&mut self, context: &egui::Context) {
        if self.download.is_none() && !self.downloaded {
            self.download = Some(crate::download::start(&self.source, self.waker()));
        }
        if let Some(result) = self.download.as_mut().and_then(Download::poll) {
            self.download = None;
            self.downloaded = true;
            match result {
                Ok(found) => {
                    self.found = found;
                    self.error = None;
                }
                Err(error) => {
                    self.found.clear();
                    self.error = Some(error);
                }
            }
        }
        self.settle();
        self.raster(context);
    }

    fn raster(&mut self, context: &egui::Context) {
        let waker = self.waker();
        self.paintings.settle(context, &waker);
    }

    fn waker(&self) -> Waker {
        self.editing
            .as_ref()
            .map(|editing| editing.host.waker())
            .unwrap_or_default()
    }

    fn refresh(&mut self) {
        self.download = None;
        self.downloaded = false;
    }

    fn editable(&self) -> bool {
        self.editing
            .as_ref()
            .is_some_and(|editing| editing.host.editable())
    }

    fn select(&mut self, path: &str) {
        self.selected = Some(path.to_owned());
        self.showing = Showing::Current;
        self.frame = 0;
        self.playing = false;
        self.advanced = None;
        self.view.fit();
    }

    fn approve(&mut self, path: &str) {
        self.showing = Showing::Current;
        self.pending = Some(path.to_owned());
        self.settle();
    }

    fn settle(&mut self) {
        let Some(path) = self.pending.clone() else {
            return;
        };
        if self.approve_now(&path) {
            self.pending = None;
        }
    }

    fn approve_now(&self, path: &str) -> bool {
        if !self.editable() {
            return true;
        }
        let Some(editing) = &self.editing else {
            return true;
        };
        let Some(painting) = self.found.iter().find(|found| found.path == path) else {
            return true;
        };
        let Some(review) = editing.block.read() else {
            return false;
        };
        let approved = review
            .approval(path)
            .and_then(|approved| approved.snapshot.as_direct());
        drop(review);
        let snapshot = PaintSnapshot::new(path, painting.data.clone());
        let reference = match approved {
            Some(id) => {
                let block = editing.client.get_block::<PaintSnapshot>(id);
                if block.read().is_none() {
                    return false;
                }
                block.operate(PaintSnapshotOperation::Replace { snapshot });
                BlockRef::Direct(id)
            }
            None => {
                let created = editing.client.create_block(snapshot);
                created.set_parent(BlockParent::Uuid(editing.block.id()));
                BlockRef::Direct(created.id())
            }
        };
        editing.block.operate(PaintReviewOperation::Approve {
            painting: ApprovedPainting {
                path: path.to_owned(),
                hash: painting.hash.clone(),
                snapshot: reference,
            },
        });
        true
    }

    fn unapprove(&mut self, path: &str) {
        if self.pending.as_deref() == Some(path) {
            self.pending = None;
        }
        self.showing = Showing::Current;
        if !self.editable() {
            return;
        }
        let Some(approval) = self.approval(path) else {
            return;
        };
        let Some(editing) = &self.editing else {
            return;
        };
        if let Some(id) = approval.snapshot.as_direct() {
            editing
                .client
                .get_block::<PaintSnapshot>(id)
                .set_parent(BlockParent::Orphaned);
        }
        editing.block.operate(PaintReviewOperation::Forget {
            path: path.to_owned(),
        });
    }

    fn hash(&self, path: &str, showing: Showing) -> Result<String, Option<String>> {
        match showing {
            Showing::Approved => self
                .approval(path)
                .map(|approved| approved.hash)
                .ok_or_else(|| Some(format!("{path} has never been approved"))),
            _ => self
                .found
                .iter()
                .find(|found| found.path == path)
                .map(|painting| painting.hash.clone())
                .ok_or_else(|| Some(format!("{path} is not on the {BRANCH} branch"))),
        }
    }

    fn data(&self, path: &str, showing: Showing) -> Result<Vec<u8>, Option<String>> {
        match showing {
            Showing::Approved => {
                let approval = self
                    .approval(path)
                    .ok_or_else(|| Some(format!("{path} has never been approved")))?;
                let id = approval.snapshot.as_direct().ok_or_else(|| {
                    Some("the approved painting is not on this workspace".to_owned())
                })?;
                let editing = self.editing.as_ref().ok_or(None)?;
                let snapshot = editing.client.get_block::<PaintSnapshot>(id);
                let data = snapshot.read().ok_or(None)?.data().to_vec();
                Ok(data)
            }
            _ => self
                .found
                .iter()
                .find(|found| found.path == path)
                .map(|painting| painting.data.clone())
                .ok_or_else(|| Some(format!("{path} is not on the {BRANCH} branch"))),
        }
    }

    fn request(&mut self, path: &str) {
        for wanted in self.showing.wanted() {
            let Ok(hash) = self.hash(path, wanted) else {
                continue;
            };
            if self.paintings.holds(&hash) {
                continue;
            }
            let Ok(data) = self.data(path, wanted) else {
                continue;
            };
            self.paintings.want(&hash, data);
        }
    }

    fn count(&self, path: &str, showing: Showing) -> usize {
        let counts = showing
            .wanted()
            .into_iter()
            .filter_map(|wanted| self.hash(path, wanted).ok())
            .filter_map(|hash| self.paintings.count(&hash));
        let count = match showing.both() {
            true => counts.min(),
            false => counts.take(1).next(),
        };
        count.unwrap_or(1).max(1)
    }

    fn change(&mut self, path: &str) -> Option<(String, Option<usize>)> {
        let current_hash = self.hash(path, Showing::Current).ok()?;
        let approved_hash = self.hash(path, Showing::Approved).ok()?;
        let key = format!("{approved_hash}\u{1}{current_hash}");
        if self.change.as_ref().is_none_or(|(seen, _)| *seen != key) {
            let approved = self.data(path, Showing::Approved).ok()?;
            let current = self.data(path, Showing::Current).ok()?;
            let change =
                crate::render::change(&approved, &current).unwrap_or_else(|error| Change {
                    description: format!("the paintings could not be compared: {error}"),
                    frame: None,
                });
            self.change = Some((key, change));
        }
        self.change
            .as_ref()
            .map(|(_, change)| (change.description.clone(), change.frame))
    }

    fn frames_ui(&mut self, ui: &mut egui::Ui, count: usize, changed: Option<usize>) {
        ui.horizontal(|ui| {
            let step = |ui: &mut egui::Ui, icon: &str, id: &str, enabled: bool| {
                ui.add_enabled(enabled, egui::Button::new(icon))
                    .test_id(id)
                    .clicked()
            };
            if step(
                ui,
                ICON_CHEVRON_LEFT.codepoint,
                "paint_review.frame.previous",
                self.frame > 0,
            ) {
                self.frame -= 1;
                self.playing = false;
            }
            let playing = self.playing;
            let icon = if playing { ICON_PAUSE } else { ICON_PLAY_ARROW };
            if step(ui, icon.codepoint, "paint_review.frame.play", true) {
                self.playing = !playing;
                self.advanced = None;
            }
            if step(
                ui,
                ICON_CHEVRON_RIGHT.codepoint,
                "paint_review.frame.next",
                self.frame + 1 < count,
            ) {
                self.frame += 1;
                self.playing = false;
            }
            let slider = ui
                .add(egui::Slider::new(&mut self.frame, 0..=count - 1).show_value(false))
                .test_id("paint_review.frame.at");
            if slider.changed() {
                self.playing = false;
            }
            ui.weak(format!("Frame {} of {count}", self.frame + 1));
            if let Some(changed) = changed.filter(|changed| *changed != self.frame) {
                if ui
                    .button(format!("{} Changed frame", ICON_DIFFERENCE.codepoint))
                    .test_id("paint_review.frame.changed")
                    .clicked()
                {
                    self.frame = changed;
                    self.playing = false;
                }
            }
        });
    }

    fn zoom_ui(&mut self, ui: &mut egui::Ui, loading: Option<(usize, usize)>) {
        ui.horizontal(|ui| {
            if ui
                .button(ICON_ZOOM_OUT.codepoint)
                .test_id("paint_review.zoom.out")
                .clicked()
            {
                self.view.zoom_out();
            }
            if ui
                .button(ICON_ZOOM_IN.codepoint)
                .test_id("paint_review.zoom.in")
                .clicked()
            {
                self.view.zoom_in();
            }
            if ui
                .selectable_label(
                    self.view.fitting(),
                    format!("{} Fit", ICON_FIT_SCREEN.codepoint),
                )
                .test_id("paint_review.zoom.fit")
                .clicked()
            {
                self.view.fit();
            }
            if ui
                .selectable_label(
                    !self.view.fitting() && (self.view.scale() - 1.0).abs() < f32::EPSILON,
                    format!("{} 1:1", ICON_PHOTO_SIZE_SELECT_ACTUAL.codepoint),
                )
                .test_id("paint_review.zoom.actual")
                .clicked()
            {
                self.view.set(1.0);
            }
            ui.weak(format!("{:.0}%", self.view.scale() * 100.0));
            if let Some((done, total)) = loading {
                ui.separator();
                ui.weak(format!("Rastering frame {} of {total}", done + 1));
                ui.spinner();
            }
        });
    }

    fn advance(&mut self, ui: &egui::Ui, count: usize, rendered: bool) {
        if !self.playing || count < 2 {
            self.advanced = None;
            return;
        }
        let now = ui.input(|input| input.time);
        let due = self
            .advanced
            .is_none_or(|advanced| now - advanced >= FRAME_SECONDS);
        if rendered && due {
            self.frame = (self.frame + 1) % count;
            self.advanced = Some(now);
        }
        ui.ctx()
            .request_repaint_after(Duration::from_secs_f64(FRAME_SECONDS));
    }

    fn shown(
        &mut self,
        context: &egui::Context,
        path: &str,
        showing: Showing,
        frame: usize,
    ) -> Shown {
        match showing {
            Showing::Approved | Showing::Current => self.single(path, showing, frame),
            Showing::SideBySide => self.side_by_side(path, frame),
            Showing::Difference => self.difference(context, path, frame),
        }
    }

    fn single(&mut self, path: &str, showing: Showing, frame: usize) -> Shown {
        let hash = match self.hash(path, showing) {
            Ok(hash) => hash,
            Err(error) => return Shown::Waiting(error),
        };
        match self.paintings.rendered(&hash, frame) {
            Some(Ok(rendered)) => {
                let description = rendered.description.clone();
                Shown::Ready(
                    vec![Panel {
                        label: showing.short(),
                        rendered,
                    }],
                    description,
                )
            }
            Some(Err(error)) => Shown::Failed(error),
            None => Shown::Waiting(None),
        }
    }

    fn pair(&mut self, path: &str, frame: usize) -> Result<(Rendered, Rendered), Shown> {
        let mut sides = Vec::new();
        for wanted in [Showing::Approved, Showing::Current] {
            let hash = match self.hash(path, wanted) {
                Ok(hash) => hash,
                Err(error) => return Err(Shown::Waiting(error)),
            };
            match self.paintings.rendered(&hash, frame) {
                Some(Ok(rendered)) => sides.push(rendered),
                Some(Err(error)) => return Err(Shown::Failed(error)),
                None => return Err(Shown::Waiting(None)),
            }
        }
        let current = sides.pop().expect("both sides were rendered");
        let approved = sides.pop().expect("both sides were rendered");
        Ok((approved, current))
    }

    fn side_by_side(&mut self, path: &str, frame: usize) -> Shown {
        let (approved, current) = match self.pair(path, frame) {
            Ok(pair) => pair,
            Err(shown) => return shown,
        };
        let description = match approved.description == current.description {
            true => current.description.clone(),
            false => format!(
                "{} before, {} now",
                approved.description, current.description
            ),
        };
        Shown::Ready(
            vec![
                Panel {
                    label: Showing::Approved.short(),
                    rendered: approved,
                },
                Panel {
                    label: Showing::Current.short(),
                    rendered: current,
                },
            ],
            description,
        )
    }

    fn difference(&mut self, context: &egui::Context, path: &str, frame: usize) -> Shown {
        let (approved, current) = match self.pair(path, frame) {
            Ok(pair) => pair,
            Err(shown) => return shown,
        };
        let key = format!(
            "\u{1}difference\u{1}{}\u{1}{}",
            self.hash(path, Showing::Approved).unwrap_or_default(),
            self.hash(path, Showing::Current).unwrap_or_default(),
        );
        let count = self.count(path, Showing::Difference);
        let painted = || Ok(crate::render::difference(&approved.image, &current.image));
        match self
            .paintings
            .computed(context, &key, frame, count, painted)
        {
            Ok(rendered) => {
                let description = rendered.description.clone();
                Shown::Ready(
                    vec![Panel {
                        label: "Difference".to_owned(),
                        rendered,
                    }],
                    description,
                )
            }
            Err(error) => Shown::Failed(error),
        }
    }
}

impl block_editor_plugin::App for PaintReviewApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        let block = client.get_block::<PaintReview>(block_id);
        self.editing = Some(Editing {
            host,
            client,
            block,
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
        Ok(client.create_block(PaintReview::new()).id())
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        let selected = self.selected.clone();
        let status = self
            .entries()
            .zip(selected.as_ref())
            .and_then(|(entries, path)| self.status(&entries, path));
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.download.is_none(),
                    egui::Button::new(format!("{} Refresh", ICON_REFRESH.codepoint)),
                )
                .test_id("paint_review.refresh")
                .clicked()
            {
                self.refresh();
            }
            if self.download.is_some() {
                ui.spinner();
            }
            ui.separator();
            let editable = self.editable();
            let approvable = editable
                && self.pending.is_none()
                && matches!(status, Some(Status::New | Status::Modified));
            if ui
                .add_enabled(
                    approvable,
                    egui::Button::new(format!("{} Approve", ICON_CHECK.codepoint)),
                )
                .test_id("paint_review.approve")
                .clicked()
            {
                if let Some(path) = &selected {
                    self.approve(path);
                }
            }
            let unapprovable = editable
                && matches!(
                    status,
                    Some(Status::Modified | Status::Removed | Status::Unchanged)
                );
            if ui
                .add_enabled(
                    unapprovable,
                    egui::Button::new(format!("{} Unapprove", ICON_DELETE.codepoint)),
                )
                .test_id("paint_review.unapprove")
                .clicked()
            {
                if let Some(path) = &selected {
                    self.unapprove(path);
                }
            }
            if status == Some(Status::Modified) {
                ui.separator();
                ui.selectable_value(&mut self.showing, Showing::Approved, "Approved")
                    .test_id("paint_review.view.approved");
                ui.selectable_value(&mut self.showing, Showing::Current, "Current")
                    .test_id("paint_review.view.current");
                ui.selectable_value(
                    &mut self.showing,
                    Showing::Difference,
                    format!("{} Difference", ICON_COMPARE.codepoint),
                )
                .test_id("paint_review.view.difference");
                ui.selectable_value(
                    &mut self.showing,
                    Showing::SideBySide,
                    format!("{} Side by side", ICON_VERTICAL_SPLIT.codepoint),
                )
                .test_id("paint_review.view.side_by_side");
            }
        });
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        if let Some(error) = self.error.clone() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        let Some(entries) = self.entries() else {
            ui.spinner();
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            for status in Status::ALL {
                let group: Vec<&Entry> = entries
                    .iter()
                    .filter(|entry| entry.status == status)
                    .collect();
                if group.is_empty() {
                    continue;
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} {} ({})",
                        status.icon(),
                        status.label(),
                        group.len()
                    ))
                    .strong(),
                );
                for entry in group {
                    let selected = self.selected.as_deref() == Some(entry.path.as_str());
                    let clicked = ui
                        .selectable_label(selected, entry.path.clone())
                        .test_id(&format!("paint_review.entry.{}", entry.path))
                        .clicked();
                    if clicked {
                        let path = entry.path.clone();
                        self.select(&path);
                    }
                }
            }
            if entries.is_empty() {
                if self.download.is_some() {
                    ui.spinner();
                } else {
                    ui.weak(format!("No paintings on the {BRANCH} branch"));
                }
            }
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll(ui.ctx());
        let Some(entries) = self.entries() else {
            ui.spinner();
            return;
        };
        let Some(path) = self.selected.clone() else {
            ui.centered_and_justified(|ui| {
                ui.weak("Choose a painting to review it");
            });
            return;
        };
        let Some(status) = self.status(&entries, &path) else {
            self.selected = None;
            return;
        };
        let showing = self.showing(status);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(path.clone()).strong());
            ui.weak(status.label());
            ui.weak(showing.label());
        });

        self.request(&path);
        self.raster(ui.ctx());

        let mut changed = None;
        if self.pending.is_some() {
            ui.weak("Waiting for the painting you approved before to arrive");
        } else if status == Status::Modified {
            if let Some((description, frame)) = self.change(&path) {
                if showing != Showing::Difference {
                    ui.weak(description);
                }
                changed = frame;
            }
        }

        let count = self.count(&path, showing);
        self.frame = self.frame.min(count - 1);
        if count > 1 {
            self.frames_ui(ui, count, changed.filter(|frame| *frame < count));
        }
        let loading = self
            .hash(&path, showing)
            .ok()
            .and_then(|hash| self.paintings.loading(&hash));
        self.zoom_ui(ui, loading);

        let shown = self.shown(ui.ctx(), &path, showing, self.frame);
        let ready = match shown {
            Shown::Ready(panels, description) => {
                ui.weak(description);
                self.view.show(ui, &panels);
                true
            }
            Shown::Failed(error) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
                true
            }
            Shown::Waiting(error) => {
                waiting(ui, error);
                false
            }
        };
        self.advance(ui, count, ready);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

fn waiting(ui: &mut egui::Ui, error: Option<String>) {
    ui.centered_and_justified(|ui| match error {
        Some(error) => {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        None => {
            ui.spinner();
        }
    });
}
