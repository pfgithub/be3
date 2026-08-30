use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use block::{BlockReference, BlockReferenceList};
use block_client::{
    blocks::version_control_data::{CommitId, VersionControlData},
    blocks::version_control_worktree::VersionControlWorktree,
    version_control_checkout::{checkout_worktree, worktree_is_clean, CheckoutOutcome},
    version_control_commit::{commit_worktree, CommitOutcome},
    BlockClient, BlockHandle, ReferenceList,
};
use eframe::egui;
use egui_material_icons::{
    icons::{
        ICON_ALT_ROUTE, ICON_CHECK_CIRCLE, ICON_COMMIT, ICON_DELETE_SWEEP, ICON_FOLDER_OPEN,
        ICON_REFRESH, ICON_SWAP_HORIZ, ICON_SYNC, ICON_UNDO, ICON_WARNING,
    },
    MaterialIcon,
};
use uuid::Uuid;

use crate::platform;

use super::{
    BlockEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess, EditorAction,
    EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 480.0;
const MEMBER_ROW_HEIGHT: f32 = 24.0;

impl EditorKind for VersionControlWorktreeEditor {
    type Block = VersionControlWorktree;

    const DISPLAY_NAME: &'static str = "Worktree";
    const ICON: MaterialIcon = ICON_FOLDER_OPEN;
    const CAN_ADD_CHILD: bool = true;
    const CAN_DELETE_CHILD: bool = true;

    fn open(client: &BlockClient, block: BlockHandle<VersionControlWorktree>) -> Self {
        Self::new(client, block)
    }
}

pub(super) struct VersionControlWorktreeEditor {
    block: BlockHandle<VersionControlWorktree>,
    data: Option<BlockHandle<VersionControlData>>,
    references: ReferenceList,
    commit_message: String,
    dirty: Option<bool>,
    dirty_request: Option<platform::RequestResult<Option<bool>>>,
    commit_request: Option<platform::RequestResult<Option<CommitOutcome>>>,
    checkout_request: Option<platform::RequestResult<Option<CheckoutOutcome>>>,
    checkout_target: Option<CommitId>,
    awaiting_discard: Option<CommitId>,
    error: Option<String>,
}

impl VersionControlWorktreeEditor {
    fn new(client: &BlockClient, block: BlockHandle<VersionControlWorktree>) -> Self {
        let references = client.watch_references(BlockReferenceList::References(block.id()));
        Self {
            block,
            data: None,
            references,
            commit_message: String::new(),
            dirty: None,
            dirty_request: None,
            commit_request: None,
            checkout_request: None,
            checkout_target: None,
            awaiting_discard: None,
            error: None,
        }
    }

    fn ensure_data(&mut self, client: &BlockClient) {
        let Some(repo_id) = self.block.read().map(|worktree| worktree.repo()) else {
            return;
        };
        if self.data.as_ref().is_none_or(|data| data.id() != repo_id) {
            self.data = Some(client.get_block::<VersionControlData>(repo_id));
        }
    }

    fn sync(&mut self, editors: &mut EditorAccess<'_>) -> Arc<BlockClient> {
        let client = editors.client_handle();
        self.poll(&client);
        self.ensure_data(editors.client());
        if self.dirty.is_none() && self.dirty_request.is_none() {
            self.refresh_dirty(&client);
        }
        client
    }

    fn refresh_dirty(&mut self, client: &Arc<BlockClient>) {
        if self.dirty_request.is_some() {
            return;
        }
        let client = Arc::clone(client);
        let worktree_id = self.block.id();
        self.dirty_request = Some(platform::spawn_request(async move {
            worktree_is_clean(&client, worktree_id).await
        }));
    }

    fn spawn_commit(&mut self, client: &Arc<BlockClient>, author: Uuid) {
        if self.commit_request.is_some() {
            return;
        }
        let message = self.commit_message.trim().to_owned();
        if message.is_empty() {
            return;
        }
        let client = Arc::clone(client);
        let worktree_id = self.block.id();
        let time = unix_seconds_now();
        self.commit_request = Some(platform::spawn_request(async move {
            commit_worktree(&client, worktree_id, author, time, message).await
        }));
    }

    fn spawn_checkout(&mut self, client: &Arc<BlockClient>, target: CommitId, discard: bool) {
        if self.checkout_request.is_some() {
            return;
        }
        self.checkout_target = Some(target.clone());
        let client = Arc::clone(client);
        let worktree_id = self.block.id();
        self.checkout_request = Some(platform::spawn_request(async move {
            checkout_worktree(&client, worktree_id, target, discard).await
        }));
    }

    fn poll(&mut self, client: &Arc<BlockClient>) {
        if let Some(receiver) = &self.dirty_request {
            match receiver.try_recv() {
                Ok(result) => {
                    self.dirty = result;
                    self.dirty_request = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.dirty_request = None;
                }
            }
        }
        if let Some(receiver) = &self.commit_request {
            match receiver.try_recv() {
                Ok(outcome) => {
                    self.commit_request = None;
                    match outcome {
                        Some(outcome) if outcome.branch_advanced => {
                            self.commit_message.clear();
                            self.error = None;
                        }
                        Some(_) => {
                            self.error = Some(
                                "The branch moved before the commit landed; try again.".to_owned(),
                            );
                        }
                        None => self.error = Some("Commit failed.".to_owned()),
                    }
                    self.refresh_dirty(client);
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.commit_request = None;
                }
            }
        }
        if let Some(receiver) = &self.checkout_request {
            match receiver.try_recv() {
                Ok(outcome) => {
                    self.checkout_request = None;
                    let target = self.checkout_target.take();
                    match outcome {
                        Some(CheckoutOutcome::Applied { .. }) => {
                            self.awaiting_discard = None;
                            self.error = None;
                            self.refresh_dirty(client);
                        }
                        Some(CheckoutOutcome::Blocked) => {
                            self.awaiting_discard = target;
                        }
                        None => self.error = Some("Checkout failed.".to_owned()),
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.checkout_request = None;
                }
            }
        }
    }

    fn status_ui(&mut self, ui: &mut egui::Ui, client: &Arc<BlockClient>, author: Uuid) {
        ui.horizontal(|ui| {
            let (icon, text) = match self.dirty {
                None => (ICON_SYNC, "Checking status…"),
                Some(true) => (ICON_WARNING, "Uncommitted changes"),
                Some(false) => (ICON_CHECK_CIRCLE, "Clean"),
            };
            ui.label(format!("{} {text}", icon.codepoint));
            if ui
                .add_enabled(
                    self.dirty_request.is_none(),
                    egui::Button::new(ICON_REFRESH.codepoint),
                )
                .on_hover_text("Refresh status")
                .clicked()
            {
                self.refresh_dirty(client);
            }
        });

        ui.add(
            egui::TextEdit::singleline(&mut self.commit_message)
                .hint_text("Commit message")
                .desired_width(f32::INFINITY),
        );
        let can_commit = self.dirty == Some(true)
            && self.commit_request.is_none()
            && !self.commit_message.trim().is_empty();
        if ui
            .add_enabled(
                can_commit,
                egui::Button::new(format!("{} Commit", ICON_COMMIT.codepoint)),
            )
            .clicked()
        {
            self.spawn_commit(client, author);
        }

        let checked_out = self
            .block
            .read()
            .map(|worktree| worktree.checked_out_commit().clone());
        let can_discard = self.dirty == Some(true) && self.checkout_request.is_none();
        if ui
            .add_enabled(
                can_discard,
                egui::Button::new(format!("{} Discard changes", ICON_UNDO.codepoint)),
            )
            .on_hover_text("Revert to the last commit, discarding uncommitted changes")
            .clicked()
        {
            if let Some(checked_out) = checked_out {
                self.spawn_checkout(client, checked_out, true);
            }
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn branch_ui(&mut self, ui: &mut egui::Ui, client: &Arc<BlockClient>) {
        let Some(data) = &self.data else {
            ui.weak("Loading repository…");
            return;
        };
        let Some(data_state) = data.read() else {
            ui.weak("Loading repository…");
            return;
        };
        let branches: Vec<(String, CommitId)> = data_state
            .branches()
            .iter()
            .map(|(name, head)| (name.clone(), head.clone()))
            .collect();
        drop(data_state);
        let checked_out = self
            .block
            .read()
            .map(|worktree| worktree.checked_out_commit().clone());

        for (name, head) in &branches {
            ui.horizontal(|ui| {
                ui.label(format!("{} {name}", ICON_ALT_ROUTE.codepoint));
                ui.monospace(head.short());
                if checked_out.as_ref() == Some(head) {
                    ui.weak("checked out");
                } else if ui
                    .add_enabled(
                        self.checkout_request.is_none(),
                        egui::Button::new(format!("{} Switch", ICON_SWAP_HORIZ.codepoint)),
                    )
                    .clicked()
                {
                    self.spawn_checkout(client, head.clone(), false);
                }
            });
        }

        if let Some(target) = self.awaiting_discard.clone() {
            ui.add_space(8.0);
            ui.colored_label(
                ui.visuals().warn_fg_color,
                "This worktree has uncommitted changes.",
            );
            ui.horizontal(|ui| {
                if ui
                    .button(format!(
                        "{} Discard uncommitted changes and switch",
                        ICON_DELETE_SWEEP.codepoint
                    ))
                    .clicked()
                {
                    self.spawn_checkout(client, target.clone(), true);
                }
                if ui.button("Cancel").clicked() {
                    self.awaiting_discard = None;
                }
            });
        }
    }

    fn member_list(
        &mut self,
        ui: &mut egui::Ui,
        editors: &EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let Some(worktree) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let members: Vec<Uuid> = worktree.members().map(|(_, live_id)| live_id).collect();
        drop(worktree);
        if members.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This worktree has no content yet.");
            });
            return None;
        }

        let metadata: HashMap<Uuid, BlockReference> = self
            .references
            .read()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let mut action = None;
        for live_id in members {
            let reference = metadata.get(&live_id);
            let (label, type_name): (egui::WidgetText, String) = match reference {
                Some(reference) => (
                    super::BlockLabel::for_reference(editors.registry(), reference)
                        .widget_text(ui.style()),
                    editors
                        .registry()
                        .display_name(reference.block_type)
                        .map_or_else(|| reference.block_type.to_string(), str::to_owned),
                ),
                None => ("Loading…".into(), String::new()),
            };
            let response = ui.add_sized(
                [ui.available_width(), MEMBER_ROW_HEIGHT],
                egui::Button::new(label).right_text(type_name).truncate(),
            );
            if response.clicked() {
                if let Some(reference) = reference {
                    action = Some(EditorAction::OpenBlock {
                        id: reference.id,
                        block_type: reference.block_type,
                    });
                }
            }
        }
        action
    }
}

impl BlockEditor for VersionControlWorktreeEditor {
    fn block(&self) -> &dyn block_client::BlockHandleAccess {
        &self.block
    }

    fn direct_editor_capabilities(&self) -> DirectEditorCapabilities {
        DirectEditorCapabilities {
            allow_rotation: false,
            preserve_aspect_ratio: false,
            supports_pan_and_zoom: false,
        }
    }

    fn direct_editor_intrinsic_size(
        &mut self,
        _editors: &mut EditorAccess<'_>,
    ) -> Option<egui::Vec2> {
        let member_count = self.block.read()?.members().count().max(1);
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            MEMBER_ROW_HEIGHT * member_count as f32,
        ))
    }

    fn direct_editor_has_right_sidebar(&self, _editors: &mut EditorAccess<'_>) -> bool {
        true
    }

    fn direct_editor_right_sidebar(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        let client = self.sync(editors);
        let author = editors.client().account_id();
        ui.heading("Status");
        self.status_ui(ui, &client, author);
        ui.add_space(8.0);
        ui.separator();
        ui.heading("Branches");
        self.branch_ui(ui, &client);
        None
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        self.sync(editors);
        self.member_list(ui, editors)
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
