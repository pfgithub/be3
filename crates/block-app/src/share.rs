use block::{BlockAccess, BlockAccessEntry, WorkspaceRole};
use block_client::{BlockAccessRequest, BlockClient};
use eframe::egui;
use egui_material_icons::icons::{ICON_LOCK, ICON_PERSON, ICON_REFRESH};
use uuid::Uuid;

/// The permissions a block can be shared with, in the order they are offered.
const SHAREABLE: [BlockAccess; 4] = [
    BlockAccess::Edit,
    BlockAccess::View,
    BlockAccess::KnowExists,
    BlockAccess::None,
];

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
        });
    }

    pub fn show(&mut self, ctx: &egui::Context, client: &BlockClient) {
        let Some(state) = &mut self.open else {
            return;
        };
        state.poll();

        let mut close = false;
        let mut reload = false;
        let mut change = None;
        let mut open = true;
        egui::Window::new(format!("Share \u{201c}{}\u{201d}", state.name))
            .id(egui::Id::new("share-block"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(420.0);
                ui.weak("Workspace members you give access to can open this block.");
                ui.add_space(8.0);

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

                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for entry in &state.entries {
                            if let Some(access) =
                                show_member(ui, entry, client.account_id(), state.id)
                            {
                                change = Some((entry.account.id, access));
                            }
                        }
                    });

                if state.loaded && state.entries.is_empty() {
                    ui.weak("This workspace has no other members.");
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

        if let Some((account_id, access)) = change {
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
                            for access in SHAREABLE {
                                if ui
                                    .selectable_label(access == current, access.label())
                                    .clicked()
                                    && access != current
                                {
                                    chosen = Some(access);
                                }
                            }
                        });
                },
            );
        });
    ui.add_space(4.0);
    chosen
}
