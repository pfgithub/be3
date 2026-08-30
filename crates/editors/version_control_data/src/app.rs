use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use block::Block;
use block_client::blocks::version_control_data::{
    Commit, CommitId, VersionControlData, VersionControlDataOperation, MAIN_BRANCH,
};
use block_client::blocks::version_control_worktree::VersionControlWorktree;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ADD, ICON_ALT_ROUTE, ICON_COMMIT, ICON_PERSON, ICON_SCHEDULE,
};
use block_editor_plugin::{egui, EditorHost};
use uuid::Uuid;

const INTRINSIC_WIDTH: f32 = 640.0;
const BRANCH_ROW_HEIGHT: f32 = 26.0;
const COMMIT_ROW_HEIGHT: f32 = 44.0;
const CHROME_HEIGHT: f32 = 192.0;
const SECONDS_PER_DAY: i64 = 86_400;

struct BranchRow {
    name: String,
    head: CommitId,
    head_commit: Option<Commit>,
}

struct CommitRow {
    id: CommitId,
    commit: Commit,
}

pub struct VersionControlDataApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<VersionControlData>>,
    selected_branch: String,
    new_branch_name: String,
}

impl Default for VersionControlDataApp {
    fn default() -> Self {
        Self {
            host: None,
            client: None,
            creation: None,
            block: None,
            selected_branch: MAIN_BRANCH.to_owned(),
            new_branch_name: String::new(),
        }
    }
}

impl VersionControlDataApp {
    fn read_view(&self) -> Option<(Vec<BranchRow>, Vec<CommitRow>)> {
        let data = self.block.as_ref()?.read()?;
        let branches: Vec<BranchRow> = data
            .branches()
            .iter()
            .map(|(name, head)| BranchRow {
                name: name.clone(),
                head: head.clone(),
                head_commit: data.commit(head).cloned(),
            })
            .collect();
        let selected_head = data.branch_head(&self.selected_branch).cloned();
        let history = selected_head.map_or_else(Vec::new, |head| {
            data.ancestors(&head)
                .into_iter()
                .filter_map(|id| {
                    let commit = data.commit(&id).cloned()?;
                    Some(CommitRow { id, commit })
                })
                .collect()
        });
        Some((branches, history))
    }

    fn branch_list(&mut self, ui: &mut egui::Ui, branches: &[BranchRow]) {
        for branch in branches {
            let selected = self.selected_branch == branch.name;
            let summary = branch.head_commit.as_ref().map_or_else(
                || "unknown commit".to_owned(),
                |commit| commit.message.clone(),
            );
            let label = format!(
                "{} {}   {}   {summary}",
                ICON_ALT_ROUTE.codepoint,
                branch.name,
                branch.head.short(),
            );
            if ui
                .selectable_label(selected, label)
                .test_id(&format!("repository.branch.{}", branch.name))
                .clicked()
            {
                self.selected_branch.clone_from(&branch.name);
            }
        }
    }

    fn new_branch_row(&mut self, ui: &mut egui::Ui, branches: &[BranchRow], editable: bool) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_branch_name).hint_text("New branch name"),
            )
            .test_id("repository.new-branch-name");
            let trimmed = self.new_branch_name.trim();
            let current_head = branches
                .iter()
                .find(|branch| branch.name == self.selected_branch)
                .map(|branch| branch.head.clone());
            let can_create = editable
                && !trimmed.is_empty()
                && current_head.is_some()
                && !branches.iter().any(|branch| branch.name == trimmed);
            if ui
                .add_enabled(
                    can_create,
                    egui::Button::new(format!("{} Create branch", ICON_ADD.codepoint)),
                )
                .test_id("repository.create-branch")
                .clicked()
            {
                if let (Some(commit), Some(block)) = (current_head, self.block.as_ref()) {
                    block.operate(VersionControlDataOperation::SetBranch {
                        name: trimmed.to_owned(),
                        expected: None,
                        commit,
                    });
                    self.new_branch_name.clear();
                }
            }
        });
    }

    fn new_worktree_button(&self, ui: &mut egui::Ui, editable: bool) {
        if !ui
            .add_enabled(
                editable,
                egui::Button::new(format!("{} New worktree", ICON_ADD.codepoint)),
            )
            .on_hover_text("Create a worktree checked out against this repository")
            .test_id("repository.new-worktree")
            .clicked()
        {
            return;
        }
        let (Some(host), Some(client), Some(block)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.block.as_ref(),
        ) else {
            return;
        };
        let Some(data) = block.read() else {
            return;
        };
        let worktree = client.create_block(VersionControlWorktree::new(block.id(), &data));
        drop(data);
        host.open_block(worktree.id(), VersionControlWorktree::TYPE_ID);
    }

    fn commit_history(&self, ui: &mut egui::Ui, history: &[CommitRow]) {
        if history.is_empty() {
            ui.weak("This branch has no commits yet.");
            return;
        }
        for row in history {
            ui.horizontal(|ui| {
                ui.label(ICON_COMMIT.codepoint);
                ui.strong(&row.commit.message);
            });
            ui.horizontal(|ui| {
                ui.monospace(row.id.short());
                ui.weak(format!(
                    "{} {}",
                    ICON_PERSON.codepoint,
                    short_author(row.commit.author)
                ));
                ui.weak(format!(
                    "{} {}",
                    ICON_SCHEDULE.codepoint,
                    format_commit_time(row.commit.time)
                ));
            });
            ui.separator();
        }
    }
}

impl block_editor_plugin::App for VersionControlDataApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        let author = client.account_id();
        let data = VersionControlData::new(author, unix_seconds_now());
        Ok(client.create_block(data).id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let (branches, history) = self.read_view()?;
        let height = CHROME_HEIGHT
            + BRANCH_ROW_HEIGHT * branches.len().max(1) as f32
            + COMMIT_ROW_HEIGHT * history.len().max(1) as f32;
        Some(egui::vec2(INTRINSIC_WIDTH, height))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some((branches, history)) = self.read_view() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let editable = self.host.as_ref().is_none_or(EditorHost::editable);

        ui.heading("Branches");
        self.branch_list(ui, &branches);
        self.new_branch_row(ui, &branches, editable);
        self.new_worktree_button(ui, editable);

        ui.add_space(8.0);
        ui.heading(format!("History  ({})", self.selected_branch));
        self.commit_history(ui, &history);
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn short_author(author: Uuid) -> String {
    author
        .simple()
        .to_string()
        .chars()
        .take(CommitId::SHORT_LEN)
        .collect()
}

pub(crate) fn format_commit_time(seconds: i64) -> String {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let time_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u8, u8) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era as i32 + era as i32 * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    (year, month as u8, day as u8)
}
