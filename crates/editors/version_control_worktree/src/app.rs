use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use block::{BlockReference, BlockReferenceList};
use block_client::blocks::version_control_data::{CommitId, VersionControlData};
use block_client::blocks::version_control_worktree::VersionControlWorktree;
use block_client::version_control_checkout::{
    checkout_worktree, worktree_is_clean, CheckoutOutcome,
};
use block_client::version_control_commit::{commit_worktree, CommitOutcome};
use block_client::{BlockClient, BlockHandle, ReferenceList};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::block_ui::{BlockLabel, BlockTypes};
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ALT_ROUTE, ICON_CHECK_CIRCLE, ICON_COMMIT, ICON_DELETE_SWEEP, ICON_REFRESH,
    ICON_SWAP_HORIZ, ICON_SYNC, ICON_UNDO, ICON_WARNING,
};
use block_editor_plugin::{egui, EditorHost, Task};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 480.0;
const MEMBER_ROW_HEIGHT: f32 = 24.0;

#[derive(Default)]
pub struct VersionControlWorktreeApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<VersionControlWorktree>>,
    data: Option<BlockHandle<VersionControlData>>,
    references: Option<ReferenceList>,
    commit_message: String,
    dirty: Option<bool>,
    dirty_request: Option<Task<Option<bool>>>,
    commit_request: Option<Task<Option<CommitOutcome>>>,
    checkout_request: Option<Task<Option<CheckoutOutcome>>>,
    checkout_target: Option<CommitId>,
    awaiting_discard: Option<CommitId>,
    error: Option<String>,
}

impl VersionControlWorktreeApp {
    fn sync(&mut self) {
        self.poll();
        self.ensure_data();
        if self.dirty.is_none() && self.dirty_request.is_none() {
            self.refresh_dirty();
        }
    }

    fn ensure_data(&mut self) {
        let (Some(client), Some(block)) = (self.client.as_ref(), self.block.as_ref()) else {
            return;
        };
        let Some(repo_id) = block.read().map(|worktree| worktree.repo()) else {
            return;
        };
        if self.data.as_ref().is_none_or(|data| data.id() != repo_id) {
            self.data = Some(client.get_block::<VersionControlData>(repo_id));
        }
    }

    fn refresh_dirty(&mut self) {
        if self.dirty_request.is_some() {
            return;
        }
        let (Some(host), Some(client), Some(block)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.block.as_ref(),
        ) else {
            return;
        };
        let client = Arc::clone(client);
        let worktree_id = block.id();
        self.dirty_request =
            Some(host.spawn(async move { worktree_is_clean(&client, worktree_id).await }));
    }

    fn spawn_commit(&mut self, author: Uuid) {
        if self.commit_request.is_some() {
            return;
        }
        let message = self.commit_message.trim().to_owned();
        if message.is_empty() {
            return;
        }
        let (Some(host), Some(client), Some(block)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.block.as_ref(),
        ) else {
            return;
        };
        let client = Arc::clone(client);
        let worktree_id = block.id();
        let time = unix_seconds_now();
        self.commit_request = Some(host.spawn(async move {
            commit_worktree(&client, worktree_id, author, time, message).await
        }));
    }

    fn spawn_checkout(&mut self, target: CommitId, discard: bool) {
        if self.checkout_request.is_some() {
            return;
        }
        let (Some(host), Some(client), Some(block)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.block.as_ref(),
        ) else {
            return;
        };
        let client = Arc::clone(client);
        let worktree_id = block.id();
        self.checkout_target = Some(target.clone());
        self.checkout_request =
            Some(host.spawn(async move {
                checkout_worktree(&client, worktree_id, target, discard).await
            }));
    }

    fn poll(&mut self) {
        if let Some(result) = finish(&mut self.dirty_request) {
            self.dirty = result.flatten();
        }
        if let Some(outcome) = finish(&mut self.commit_request) {
            match outcome.flatten() {
                Some(outcome) if outcome.branch_advanced => {
                    self.commit_message.clear();
                    self.error = None;
                }
                Some(_) => {
                    self.error =
                        Some("The branch moved before the commit landed; try again.".to_owned());
                }
                None => self.error = Some("Commit failed.".to_owned()),
            }
            self.dirty = None;
            self.refresh_dirty();
        }
        if let Some(outcome) = finish(&mut self.checkout_request) {
            let target = self.checkout_target.take();
            match outcome.flatten() {
                Some(CheckoutOutcome::Applied { .. }) => {
                    self.awaiting_discard = None;
                    self.error = None;
                    self.dirty = None;
                    self.refresh_dirty();
                }
                Some(CheckoutOutcome::Blocked) => self.awaiting_discard = target,
                None => self.error = Some("Checkout failed.".to_owned()),
            }
        }
    }

    fn status_ui(&mut self, ui: &mut egui::Ui, author: Uuid, editable: bool) {
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
                .test_id("worktree.refresh")
                .clicked()
            {
                self.refresh_dirty();
            }
        });

        ui.add(
            egui::TextEdit::singleline(&mut self.commit_message)
                .hint_text("Commit message")
                .desired_width(f32::INFINITY),
        )
        .test_id("worktree.commit-message");
        let can_commit = editable
            && self.dirty == Some(true)
            && self.commit_request.is_none()
            && !self.commit_message.trim().is_empty();
        if ui
            .add_enabled(
                can_commit,
                egui::Button::new(format!("{} Commit", ICON_COMMIT.codepoint)),
            )
            .test_id("worktree.commit")
            .clicked()
        {
            self.spawn_commit(author);
        }

        let checked_out = self
            .block
            .as_ref()
            .and_then(|block| block.read())
            .map(|worktree| worktree.checked_out_commit().clone());
        let can_discard = editable && self.dirty == Some(true) && self.checkout_request.is_none();
        if ui
            .add_enabled(
                can_discard,
                egui::Button::new(format!("{} Discard changes", ICON_UNDO.codepoint)),
            )
            .on_hover_text("Revert to the last commit, discarding uncommitted changes")
            .test_id("worktree.discard")
            .clicked()
        {
            if let Some(checked_out) = checked_out {
                self.spawn_checkout(checked_out, true);
            }
        }

        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }

    fn branch_ui(&mut self, ui: &mut egui::Ui, editable: bool) {
        let Some(data_state) = self.data.as_ref().and_then(|data| data.read()) else {
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
            .as_ref()
            .and_then(|block| block.read())
            .map(|worktree| worktree.checked_out_commit().clone());

        for (name, head) in &branches {
            ui.horizontal(|ui| {
                ui.label(format!("{} {name}", ICON_ALT_ROUTE.codepoint));
                ui.monospace(head.short());
                if checked_out.as_ref() == Some(head) {
                    ui.weak("checked out");
                } else if ui
                    .add_enabled(
                        editable && self.checkout_request.is_none(),
                        egui::Button::new(format!("{} Switch", ICON_SWAP_HORIZ.codepoint)),
                    )
                    .test_id(&format!("worktree.switch.{name}"))
                    .clicked()
                {
                    self.spawn_checkout(head.clone(), false);
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
                    .test_id("worktree.discard-and-switch")
                    .clicked()
                {
                    self.spawn_checkout(target.clone(), true);
                }
                if ui
                    .button("Cancel")
                    .test_id("worktree.cancel-switch")
                    .clicked()
                {
                    self.awaiting_discard = None;
                }
            });
        }
    }

    fn member_list(&mut self, ui: &mut egui::Ui) {
        let (Some(host), Some(block)) = (self.host.as_ref(), self.block.as_ref()) else {
            return;
        };
        let Some(worktree) = block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let members: Vec<Uuid> = worktree.members().map(|(_, live_id)| live_id).collect();
        drop(worktree);
        if members.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak("This worktree has no content yet.");
            });
            return;
        }

        let metadata: HashMap<Uuid, BlockReference> = self
            .references
            .as_ref()
            .map(ReferenceList::read)
            .unwrap_or_default()
            .into_iter()
            .map(|reference| (reference.id, reference))
            .collect();
        let types = host.block_types();
        for live_id in members {
            let reference = metadata.get(&live_id);
            let (label, type_name): (egui::WidgetText, String) = match reference {
                Some(reference) => (
                    BlockLabel::for_reference(types.as_ref(), reference).widget_text(ui.style()),
                    types
                        .display_name(reference.block_type)
                        .map_or_else(|| reference.block_type.to_string(), str::to_owned),
                ),
                None => ("Loading…".into(), String::new()),
            };
            let response = ui
                .add_sized(
                    [ui.available_width(), MEMBER_ROW_HEIGHT],
                    egui::Button::new(label).right_text(type_name).truncate(),
                )
                .test_id(&format!("worktree.member.{live_id}"));
            if response.clicked() {
                if let Some(reference) = reference {
                    host.open_block(reference.id, reference.block_type);
                }
            }
        }
    }
}

impl block_editor_plugin::App for VersionControlWorktreeApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.references = Some(client.watch_references(BlockReferenceList::References(block_id)));
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let members = self.block.as_ref()?.read()?.members().count().max(1);
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            MEMBER_ROW_HEIGHT * members as f32,
        ))
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        self.sync();
        let Some(client) = self.client.clone() else {
            ui.spinner();
            return;
        };
        let editable = self.host.as_ref().is_none_or(EditorHost::editable);
        let author = client.account_id();
        ui.heading("Status");
        self.status_ui(ui, author, editable);
        ui.add_space(8.0);
        ui.separator();
        ui.heading("Branches");
        self.branch_ui(ui, editable);
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync();
        self.member_list(ui);
    }
}

fn finish<T>(slot: &mut Option<Task<T>>) -> Option<Option<T>> {
    let task = slot.as_mut()?;
    let result = task.take();
    if !task.finished() {
        return None;
    }
    *slot = None;
    Some(result)
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}
