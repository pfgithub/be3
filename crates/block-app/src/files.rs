use block::Block;
use block_client::blocks::settings::Settings;
use eframe::egui;
use egui_material_icons::icons::ICON_CHECK;
use uuid::Uuid;

use crate::{editors::SidebarDragSource, performance, BlockApp, PendingDestructiveAction};

impl BlockApp {
    fn open_settings(&mut self) {
        let Some(id) = self
            .root_settings
            .ensure(&self.client)
            .map(|settings| settings.id())
        else {
            return;
        };
        self.open_tab(id, Settings::TYPE_ID);
    }

    pub(crate) fn can_edit_block(&self, id: Uuid) -> bool {
        self.client.block_access(id).can_edit()
    }

    fn can_delete_from(&self, source: SidebarDragSource) -> bool {
        match source {
            SidebarDragSource::Root | SidebarDragSource::Orphaned => true,
            SidebarDragSource::Block(id) => {
                self.block_type_of(id)
                    .is_some_and(|block_type| self.registry.can_delete_child(block_type))
                    && self.can_edit_block(id)
            }
        }
    }

    pub(crate) fn can_move_out_of(
        &self,
        source: SidebarDragSource,
        child: Uuid,
        is_reference: bool,
    ) -> bool {
        self.can_delete_from(source) && (is_reference || self.can_edit_block(child))
    }

    pub(crate) fn show_status_bar(&mut self, ui: &mut egui::Ui) {
        let debug = self.client.network_debug_snapshot();
        ui.horizontal(|ui| {
            if debug.changes_saved {
                ui.horizontal(|ui| {
                    ui.small(ICON_CHECK);
                    ui.small("All changes saved");
                });
            } else {
                ui.spinner();
                ui.small("Submitting changes\u{2026}");
            }
            if let Some(frame) = performance::last_frame() {
                ui.separator();
                let cause = frame
                    .causes
                    .first()
                    .map(String::as_str)
                    .unwrap_or("unknown cause");
                let milliseconds = frame.duration.as_secs_f64() * 1_000.0;
                let response = ui.small(format!(
                    "Frame {}: {milliseconds:.3} ms from {cause}",
                    frame.number
                ));
                if frame.causes.len() > 1 {
                    response.on_hover_text(frame.causes.join("\n"));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.menu_button("More", |ui| {
                    if ui.button("Settings").clicked() {
                        self.open_settings();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Client").clicked() {
                        self.client_debug_open = true;
                        ui.close();
                    }
                    if ui.button("Network").clicked() {
                        self.network_debug_open = true;
                        ui.close();
                    }
                    if ui.button("Performance").clicked() {
                        performance::open();
                        ui.close();
                    }
                    if ui.button("Plugins").clicked() {
                        crate::debug::plugins::open();
                        ui.close();
                    }
                    if ui.button("Version").clicked() {
                        crate::debug::version::open();
                        ui.close();
                    }
                    if ui.button("egui Inspection").clicked() {
                        crate::debug::inspect::open(crate::debug::inspect::Window::Inspection);
                        ui.close();
                    }
                    if ui.button("egui Memory").clicked() {
                        crate::debug::inspect::open(crate::debug::inspect::Window::Memory);
                        ui.close();
                    }
                    if ui.button("egui Style").clicked() {
                        crate::debug::inspect::open(crate::debug::inspect::Window::Style);
                        ui.close();
                    }
                    if ui.button("egui Textures").clicked() {
                        crate::debug::inspect::open(crate::debug::inspect::Window::Textures);
                        ui.close();
                    }
                    crate::debug::inspect::debug_on_hover_toggle(ui);
                    if ui.button("Terminal").clicked() {
                        crate::debug::terminal::open();
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Workspace");
                    if let Some(workspace) = &self.workspace {
                        ui.small(&workspace.name);
                    }
                    if ui.button("Invite member").clicked() {
                        self.invite_open = true;
                        ui.close();
                    }
                    if ui.button("Switch workspace").clicked() {
                        if debug.changes_saved {
                            self.scheduled_workspace_list = true;
                        } else {
                            self.pending_destructive_action =
                                Some(PendingDestructiveAction::ChooseWorkspace);
                        }
                        ui.close();
                    }
                    ui.separator();
                    ui.strong("Accounts");
                    ui.small(format!("Signed in as {}", self.account.name));
                    ui.small(self.account.id.to_string());
                    for account in self.accounts.clone() {
                        if ui
                            .selectable_label(account == self.account, &account.name)
                            .on_hover_text(account.id.to_string())
                            .clicked()
                        {
                            self.request_account_switch(account);
                            ui.close();
                        }
                    }
                    if ui
                        .add_enabled(debug.changes_saved, egui::Button::new("Manage accounts"))
                        .on_disabled_hover_text("Wait for changes to finish saving")
                        .clicked()
                    {
                        self.signed_in = false;
                        if let Err(error) = self.app_state.clear_active_account() {
                            self.account_error = Some(error.to_string());
                        }
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("About").clicked() {
                        self.about_open = true;
                        ui.close();
                    }
                });
            });
        });
    }
}
