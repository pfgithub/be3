use std::sync::Arc;

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
    egui::{self, emath::GuiRounding as _},
    egui_material_icons::icons::{
        ICON_CHECK, ICON_DELETE, ICON_DIFFERENCE, ICON_DONE_ALL, ICON_FIBER_NEW, ICON_REFRESH,
    },
    EditorHost, Waker,
};
use uuid::Uuid;

use crate::download::{Download, Painting, Source, BRANCH};
use crate::render::Rendered;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(960.0, 640.0);

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Showing {
    Approved,
    #[default]
    Current,
}

impl Showing {
    fn key(self) -> &'static str {
        match self {
            Self::Approved => "approved",
            Self::Current => "current",
        }
    }

    fn label(self) -> String {
        match self {
            Self::Approved => "the painting you approved".to_owned(),
            Self::Current => format!("the painting on the {BRANCH} branch"),
        }
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
    view: Option<(String, Result<Rendered, String>)>,
    change: Option<(String, String)>,
    pending: Option<String>,
}

impl PaintReviewApp {
    #[cfg(test)]
    pub fn review(&mut self, source: Source) {
        self.source = source;
        self.refresh();
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

    fn poll(&mut self) {
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
        self.view = None;
        self.change = None;
    }

    fn editable(&self) -> bool {
        self.editing
            .as_ref()
            .is_some_and(|editing| editing.host.editable())
    }

    fn approve(&mut self, path: &str) {
        self.view = None;
        self.change = None;
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

    fn forget(&mut self, path: &str) {
        self.view = None;
        self.change = None;
        if self.selected.as_deref() == Some(path) {
            self.selected = None;
        }
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

    fn painting(&self, path: &str, showing: Showing) -> Result<(String, Vec<u8>), Option<String>> {
        match showing {
            Showing::Current => {
                let painting = self
                    .found
                    .iter()
                    .find(|found| found.path == path)
                    .ok_or_else(|| Some(format!("{path} is not on the {BRANCH} branch")))?;
                Ok((painting.hash.clone(), painting.data.clone()))
            }
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
                Ok((approval.hash.clone(), data))
            }
        }
    }

    fn change_description(&mut self, path: &str) -> Option<String> {
        let (current_hash, current) = self.painting(path, Showing::Current).ok()?;
        let (approved_hash, approved) = self.painting(path, Showing::Approved).ok()?;
        let key = format!("{path}\u{1}{approved_hash}\u{1}{current_hash}");
        if self.change.as_ref().is_none_or(|(seen, _)| *seen != key) {
            let description = crate::render::change(&approved, &current)
                .unwrap_or_else(|error| format!("the paintings could not be compared: {error}"));
            self.change = Some((key, description));
        }
        self.change
            .as_ref()
            .map(|(_, description)| description.clone())
    }

    fn view(&mut self, ui: &mut egui::Ui, path: &str, showing: Showing) {
        let (hash, data) = match self.painting(path, showing) {
            Ok(painting) => painting,
            Err(error) => {
                ui.centered_and_justified(|ui| match error {
                    Some(error) => {
                        ui.colored_label(ui.visuals().error_fg_color, error);
                    }
                    None => {
                        ui.spinner();
                    }
                });
                return;
            }
        };
        let key = format!("{path}\u{1}{}\u{1}{hash}", showing.key());
        if self.view.as_ref().is_none_or(|(seen, _)| *seen != key) {
            let rendered = crate::render::render(ui.ctx(), "paint-review", &data);
            self.view = Some((key, rendered));
        }
        match &self.view {
            Some((_, Ok(rendered))) => {
                ui.weak(rendered.description.clone());
                paint(ui, rendered);
            }
            Some((_, Err(error))) => {
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
            None => {}
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
        self.poll();
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
            let forgettable = editable && status == Some(Status::Removed);
            if ui
                .add_enabled(
                    forgettable,
                    egui::Button::new(format!("{} Forget", ICON_DELETE.codepoint)),
                )
                .test_id("paint_review.forget")
                .clicked()
            {
                if let Some(path) = &selected {
                    self.forget(path);
                }
            }
            if status == Some(Status::Modified) {
                ui.separator();
                ui.selectable_value(&mut self.showing, Showing::Approved, "Approved")
                    .test_id("paint_review.view.approved");
                ui.selectable_value(&mut self.showing, Showing::Current, "Current")
                    .test_id("paint_review.view.current");
            }
        });
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.poll();
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
                        self.selected = Some(entry.path.clone());
                        self.showing = Showing::Current;
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
        self.poll();
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
        if self.pending.is_some() {
            ui.weak("Waiting for the painting you approved before to arrive");
        } else if status == Status::Modified {
            if let Some(description) = self.change_description(&path) {
                ui.weak(description);
            }
        }
        self.view(ui, &path, showing);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }
}

fn paint(ui: &mut egui::Ui, rendered: &Rendered) {
    let available = ui.available_size().max(egui::Vec2::splat(1.0));
    let scale = (available.x / rendered.size.x)
        .min(available.y / rendered.size.y)
        .min(1.0);
    let (viewport, _) = ui.allocate_exact_size(available, egui::Sense::hover());
    let rect = egui::Rect::from_center_size(viewport.center(), rendered.size * scale)
        .round_to_pixels(ui.pixels_per_point());
    ui.painter_at(viewport).image(
        rendered.texture.id(),
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );
}
