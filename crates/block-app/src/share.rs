use block::{Account, BlockAccess, BlockAccessEntry, WorkspaceRole};
use block_client::{BlockAccessRequest, BlockClient};
use eframe::egui;
use egui_material_icons::icons::{
    ICON_CLOSE, ICON_LOCK, ICON_PERSON, ICON_PERSON_ADD, ICON_PERSON_REMOVE, ICON_REFRESH,
    ICON_SEARCH,
};
use uuid::Uuid;

/// The permissions a block can be granted with, in the order they are offered.
const GRANTABLE: [BlockAccess; 3] = [
    BlockAccess::Edit,
    BlockAccess::View,
    BlockAccess::KnowExists,
];

/// How many matches the picker offers at once before the query has to be
/// narrowed down.
const MAX_SUGGESTIONS: usize = 6;

#[derive(Default)]
pub struct ShareDialog {
    open: Option<ShareState>,
}

struct ShareState {
    id: Uuid,
    name: String,
    request: Option<BlockAccessRequest>,
    entries: Vec<BlockAccessEntry>,
    loaded: bool,
    error: Option<String>,
    /// What has been typed into the people picker.
    query: String,
    /// Accounts that have been picked but not granted access yet.
    pending: Vec<Account>,
    /// The permission the picked accounts are about to be given.
    pending_access: BlockAccess,
}

impl ShareDialog {
    /// Opens the dialog for `id` and starts loading who can currently reach it.
    pub fn open(&mut self, client: &BlockClient, id: Uuid, name: String) {
        self.open = Some(ShareState {
            id,
            name,
            request: Some(client.request_block_access(id)),
            entries: Vec::new(),
            loaded: false,
            error: None,
            query: String::new(),
            pending: Vec::new(),
            pending_access: BlockAccess::Edit,
        });
    }

    pub fn show(&mut self, ctx: &egui::Context, client: &BlockClient) {
        let Some(state) = &mut self.open else {
            return;
        };
        state.poll();

        let mut close = false;
        let mut reload = false;
        let mut grants = Vec::new();
        let mut open = true;
        egui::Window::new(format!("Share \u{201c}{}\u{201d}", state.name))
            .id(egui::Id::new("share-block"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(460.0);

                if let Some(error) = &state.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                    ui.add_space(8.0);
                }
                if !state.loaded && state.error.is_none() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak("Loading members\u{2026}");
                    });
                }

                state.show_picker(ui, client.account_id(), &mut grants);

                ui.add_space(12.0);
                ui.strong("People with access");
                ui.add_space(4.0);
                let mut shown = 0;
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for entry in &state.entries {
                            if !has_access(entry, client.account_id()) {
                                continue;
                            }
                            shown += 1;
                            if let Some(access) =
                                show_member(ui, entry, client.account_id(), state.id)
                            {
                                grants.push((entry.account.id, access));
                            }
                        }
                    });
                if state.loaded && shown == 0 {
                    ui.weak("Nobody can open this block yet.");
                }

                ui.add_space(12.0);
                egui::Sides::new().show(
                    ui,
                    |ui| {
                        reload = ui
                            .add_enabled(
                                state.request.is_none(),
                                egui::Button::new(format!("{} Refresh", ICON_REFRESH.codepoint)),
                            )
                            .clicked();
                    },
                    |ui| close = ui.button("Done").clicked(),
                );
            });

        for (account_id, access) in grants {
            client.set_block_access(state.id, account_id, access);
            reload = true;
        }
        if reload {
            state.error = None;
            state.request = Some(client.request_block_access(state.id));
        }
        if close || !open {
            self.open = None;
        }
    }
}

impl ShareState {
    fn poll(&mut self) {
        let Some(result) = self.request.as_mut().and_then(BlockAccessRequest::poll) else {
            return;
        };
        self.request = None;
        match result {
            Ok(entries) => {
                self.entries = entries;
                self.loaded = true;
            }
            Err(error) => self.error = Some(error),
        }
    }

    /// Draws the picker that queues people up and hands them their permission,
    /// pushing every confirmed grant onto `grants`.
    fn show_picker(
        &mut self,
        ui: &mut egui::Ui,
        account_id: Uuid,
        grants: &mut Vec<(Uuid, BlockAccess)>,
    ) {
        let response = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .desired_width(f32::INFINITY)
                .hint_text(format!(
                    "{} Add people by name or email",
                    ICON_SEARCH.codepoint
                )),
        );
        let submitted =
            response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));

        let candidates = self.candidates(account_id);
        let mut picked = None;
        if submitted {
            picked = candidates.first().cloned();
            response.request_focus();
        } else if !self.query.is_empty() {
            ui.add_space(4.0);
            egui::Frame::group(ui.style())
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if candidates.is_empty() {
                        ui.weak("No matching workspace members.");
                        return;
                    }
                    for account in candidates.iter().take(MAX_SUGGESTIONS) {
                        if ui
                            .add(
                                egui::Button::new(format!(
                                    "{} {} ({})",
                                    ICON_PERSON.codepoint, account.display_name, account.email
                                ))
                                .frame(false),
                            )
                            .clicked()
                        {
                            picked = Some(account.clone());
                        }
                    }
                });
        }
        if let Some(account) = picked {
            self.pending.push(account);
            self.query.clear();
        }

        if self.pending.is_empty() {
            return;
        }
        ui.add_space(6.0);
        let mut removed = None;
        ui.horizontal_wrapped(|ui| {
            for (index, account) in self.pending.iter().enumerate() {
                egui::Frame::group(ui.style())
                    .inner_margin(egui::Margin::symmetric(6, 2))
                    .show(ui, |ui| {
                        ui.label(format!(
                            "{} {}",
                            ICON_PERSON.codepoint, account.display_name
                        ));
                        if ui
                            .small_button(ICON_CLOSE.codepoint.to_string())
                            .on_hover_text("Do not add")
                            .clicked()
                        {
                            removed = Some(index);
                        }
                    });
            }
        });
        if let Some(index) = removed {
            self.pending.remove(index);
        }

        ui.add_space(6.0);
        let mut access = self.pending_access;
        let mut add = false;
        egui::Sides::new().show(
            ui,
            |ui| {
                egui::ComboBox::from_id_salt("share-pending-access")
                    .selected_text(access.label())
                    .show_ui(ui, |ui| {
                        for option in GRANTABLE {
                            ui.selectable_value(&mut access, option, option.label());
                        }
                    });
            },
            |ui| {
                add = ui
                    .button(format!("{} Add", ICON_PERSON_ADD.codepoint))
                    .clicked();
            },
        );
        self.pending_access = access;
        if add {
            grants.extend(self.pending.drain(..).map(|account| (account.id, access)));
            self.query.clear();
        }
    }

    /// The workspace members the query matches that are not already queued up
    /// or able to reach the block.
    fn candidates(&self, account_id: Uuid) -> Vec<Account> {
        let query = self.query.trim().to_lowercase();
        self.entries
            .iter()
            .filter(|entry| !has_access(entry, account_id))
            .filter(|entry| {
                !self
                    .pending
                    .iter()
                    .any(|pending| pending.id == entry.account.id)
            })
            .filter(|entry| {
                query.is_empty()
                    || entry.account.display_name.to_lowercase().contains(&query)
                    || entry.account.email.to_lowercase().contains(&query)
            })
            .map(|entry| entry.account.clone())
            .collect()
    }
}

/// Whether the member already reaches the block, and so belongs in the list of
/// people with access rather than the picker.
fn has_access(entry: &BlockAccessEntry, account_id: Uuid) -> bool {
    matches!(entry.role, WorkspaceRole::Administrator)
        || entry.account.id == account_id
        || entry.granted.is_some()
        || entry.effective > BlockAccess::None
}

/// Draws one member's row, returning the access they were just given.
fn show_member(
    ui: &mut egui::Ui,
    entry: &BlockAccessEntry,
    account_id: Uuid,
    block_id: Uuid,
) -> Option<BlockAccess> {
    // Administrators reach every block through their workspace role, and an
    // account cannot revoke its own access, so neither row is editable.
    let fixed = match entry.role {
        WorkspaceRole::Administrator => Some("Administrators can open every block"),
        WorkspaceRole::Editor if entry.account.id == account_id => Some("This is you"),
        WorkspaceRole::Editor => None,
    };
    let mut chosen = None;
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Sides::new().shrink_left().show(
                ui,
                |ui| {
                    ui.vertical(|ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!(
                                    "{} {}",
                                    ICON_PERSON.codepoint, entry.account.display_name
                                ))
                                .strong(),
                            )
                            .truncate(),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(entry.account.email.as_str())
                                    .small()
                                    .weak(),
                            )
                            .truncate(),
                        );
                        if let Some(fixed) = fixed {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{} {fixed}", ICON_LOCK.codepoint))
                                        .small()
                                        .weak(),
                                )
                                .truncate(),
                            );
                        } else if entry.granted != Some(entry.effective) {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!(
                                        "Inherited: {}",
                                        entry.effective.label()
                                    ))
                                    .small()
                                    .weak(),
                                )
                                .truncate(),
                            );
                        }
                    });
                },
                |ui| {
                    if fixed.is_some() {
                        ui.add_enabled(false, egui::Button::new(entry.effective.label()));
                        return;
                    }
                    let current = entry.granted.unwrap_or(BlockAccess::None);
                    egui::ComboBox::from_id_salt(("share-access", block_id, entry.account.id))
                        .selected_text(current.label())
                        .show_ui(ui, |ui| {
                            for access in GRANTABLE {
                                if ui
                                    .selectable_label(access == current, access.label())
                                    .clicked()
                                    && access != current
                                {
                                    chosen = Some(access);
                                }
                            }
                            // Revoking is recorded as an explicit grant of no
                            // access, so it also blocks inherited permissions.
                            if entry.granted != Some(BlockAccess::None) {
                                ui.separator();
                                if ui
                                    .selectable_label(
                                        false,
                                        format!("{} Remove access", ICON_PERSON_REMOVE.codepoint),
                                    )
                                    .clicked()
                                {
                                    chosen = Some(BlockAccess::None);
                                }
                            }
                        });
                },
            );
        });
    ui.add_space(4.0);
    chosen
}
