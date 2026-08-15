use std::time::{SystemTime, UNIX_EPOCH};

use block::{Block, BlockParent};
use block_client::{
    blocks::version_control_data::{
        Commit, CommitId, VersionControlData, VersionControlDataOperation, MAIN_BRANCH,
    },
    blocks::version_control_worktree::VersionControlWorktree,
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_ACCOUNT_TREE, ICON_ADD, ICON_ALT_ROUTE, ICON_COMMIT, ICON_PERSON, ICON_SCHEDULE},
    MaterialIcon,
};

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 640.0;
const BRANCH_ROW_HEIGHT: f32 = 26.0;
const COMMIT_ROW_HEIGHT: f32 = 44.0;
const CHROME_HEIGHT: f32 = 192.0;
const SHORT_ID_LEN: usize = 8;
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

impl EditorKind for VersionControlDataEditor {
    type Block = VersionControlData;

    const DISPLAY_NAME: &'static str = "Repository";
    const ICON: MaterialIcon = ICON_ACCOUNT_TREE;

    fn open(_client: &BlockClient, block: BlockHandle<VersionControlData>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for VersionControlDataEditor {
    fn create(client: &BlockClient) -> Self {
        let author = client.account_id();
        let time = unix_seconds_now();
        Self::new(client.create_block(VersionControlData::new(author, time)))
    }
}

pub(super) struct VersionControlDataEditor {
    block: BlockHandle<VersionControlData>,
    selected_branch: String,
    new_branch_name: String,
}

impl VersionControlDataEditor {
    fn new(block: BlockHandle<VersionControlData>) -> Self {
        Self {
            block,
            selected_branch: MAIN_BRANCH.to_owned(),
            new_branch_name: String::new(),
        }
    }

    fn read_view(&self) -> Option<(Vec<BranchRow>, Vec<CommitRow>)> {
        let data = self.block.read()?;
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
                short_commit_id(&branch.head),
            );
            if ui.selectable_label(selected, label).clicked() {
                self.selected_branch = branch.name.clone();
            }
        }
    }

    fn new_branch_row(&mut self, ui: &mut egui::Ui, branches: &[BranchRow]) {
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.new_branch_name).hint_text("New branch name"),
            );
            let trimmed = self.new_branch_name.trim();
            let current_head = branches
                .iter()
                .find(|branch| branch.name == self.selected_branch)
                .map(|branch| branch.head.clone());
            let can_create = !trimmed.is_empty()
                && current_head.is_some()
                && !branches.iter().any(|branch| branch.name == trimmed);
            if ui
                .add_enabled(
                    can_create,
                    egui::Button::new(format!("{} Create branch", ICON_ADD.codepoint)),
                )
                .clicked()
            {
                if let Some(commit) = current_head {
                    self.block.operate(VersionControlDataOperation::SetBranch {
                        name: trimmed.to_owned(),
                        expected: None,
                        commit,
                    });
                    self.new_branch_name.clear();
                }
            }
        });
    }

    fn new_worktree_button(
        &self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
    ) -> Option<EditorAction> {
        if !ui
            .button(format!("{} New worktree", ICON_ADD.codepoint))
            .on_hover_text("Create a worktree checked out against this repository")
            .clicked()
        {
            return None;
        }
        let client = editors.client();
        let data_state = self.block.read()?;
        let worktree =
            client.create_block(VersionControlWorktree::new(self.block.id(), &data_state));
        drop(data_state);
        Some(EditorAction::OpenBlock {
            id: worktree.id(),
            block_type: VersionControlWorktree::TYPE_ID,
        })
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
                ui.monospace(short_commit_id(&row.id));
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

impl BlockEditor for VersionControlDataEditor {
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
        let (branches, history) = self.read_view()?;
        let height = CHROME_HEIGHT
            + BRANCH_ROW_HEIGHT * branches.len().max(1) as f32
            + COMMIT_ROW_HEIGHT * history.len().max(1) as f32;
        Some(egui::vec2(DIRECT_EDITOR_WIDTH, height))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some((branches, history)) = self.read_view() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };

        ui.heading("Branches");
        self.branch_list(ui, &branches);
        self.new_branch_row(ui, &branches);
        let action = self.new_worktree_button(ui, editors);

        ui.add_space(8.0);
        ui.heading(format!("History  ({})", self.selected_branch));
        self.commit_history(ui, &history);

        action
    }
}

pub(super) fn short_commit_id(id: &CommitId) -> String {
    id.as_str().chars().take(SHORT_ID_LEN).collect()
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

fn short_author(author: uuid::Uuid) -> String {
    author
        .simple()
        .to_string()
        .chars()
        .take(SHORT_ID_LEN)
        .collect()
}

fn format_commit_time(seconds: i64) -> String {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let time_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Howard Hinnant's civil-calendar algorithm: days since the Unix epoch to a
/// proleptic Gregorian year/month/day.
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

#[cfg(test)]
mod tests;
