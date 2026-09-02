use super::*;

#[derive(Default)]
pub struct LogicGridApp {
    editor: Option<LogicGridEditor>,
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    artifact: Option<ArtifactState>,
}

struct ArtifactState {
    client: Arc<BlockClient>,
    target_id: Uuid,
    target_type: Uuid,
    regeneration: Option<dynamic_artifact::CompileRegeneration>,
}

impl LogicGridApp {
    fn synced(&mut self) -> Option<&mut LogicGridEditor> {
        let client = self.client.clone();
        let client_id = self
            .host
            .as_ref()
            .map_or_else(Uuid::nil, block_editor_plugin::EditorHost::client_id);
        let editor = self.editor.as_mut()?;
        editor.sync(client.as_deref(), client_id);
        Some(editor)
    }
}

impl block_editor_plugin::App for LogicGridApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.editor = Some(LogicGridEditor::new(client.get_block(block_id)));
        self.host = Some(host);
        self.client = Some(client);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.client = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .client
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(LogicGrid::new()).id())
    }

    fn connect_artifact(
        &mut self,
        _host: EditorHost,
        client: Arc<BlockClient>,
        artifact: block_editor_plugin::Artifact,
    ) {
        self.artifact = Some(ArtifactState {
            client,
            target_id: artifact.block_id,
            target_type: artifact.block_type,
            regeneration: None,
        });
    }

    fn describe_artifact(
        &mut self,
        data: &[u8],
    ) -> Result<block_editor_plugin::ArtifactDescription, String> {
        Ok(block_editor_plugin::ArtifactDescription {
            source: dynamic_artifact::source(data)?,
            summary: dynamic_artifact::summary(data),
        })
    }

    fn artifact_settings_ui(&mut self, ui: &mut egui::Ui, data: &mut Vec<u8>) {
        dynamic_artifact::settings_ui(ui, data);
    }

    fn regenerate_artifact(&mut self, data: &[u8]) {
        let Some(artifact) = &mut self.artifact else {
            return;
        };
        artifact.regeneration = dynamic_artifact::regenerate(
            &artifact.client,
            artifact.target_id,
            artifact.target_type,
            data,
        )
        .ok();
    }

    fn poll_artifact(&mut self) -> Option<Result<(), String>> {
        let artifact = self.artifact.as_mut()?;
        let Some(regeneration) = &mut artifact.regeneration else {
            return Some(Err("this component could not be regenerated".to_owned()));
        };
        let outcome = regeneration.poll()?;
        artifact.regeneration = None;
        Some(outcome)
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = self.synced() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        if editor.take_challenge_passed() {
            editor.edit(LogicGridOperation::SetCompleted { completed: true });
        }
        if editor.block.read().is_none() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }
        editor.canvas_ui(ui);
    }

    fn toolbar_ui(&mut self, ui: &mut egui::Ui) {
        let host = self.host.clone();
        let client = self.client.clone();
        let Some(editor) = self.synced() else {
            return;
        };
        let mut compiled = None;
        ui.horizontal(|ui| {
            if ui
                .button(format!("{} Compile", ICON_BUILD.codepoint))
                .on_hover_text("Build a component other grids can call")
                .clicked()
            {
                if let Some(client) = &client {
                    compiled = editor.compile(client);
                }
            }
            if let Some(challenge) = &editor.challenge {
                ui.separator();
                ui.strong(challenge.id.name());
            }
            let errors = editor.grid.validate().len();
            if errors > 0 {
                ui.separator();
                ui.colored_label(
                    ui.visuals().error_fg_color,
                    format!("{errors} placement problems"),
                );
            }
            if let Some(error) = &editor.compile_error {
                ui.separator();
                ui.colored_label(ui.visuals().error_fg_color, error);
            }
        });
        if let (Some(host), Some((id, block_type))) = (host, compiled) {
            host.open_block(id, block_type);
        }
    }

    fn left_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        let Some(editor) = self.synced() else {
            return;
        };
        ui.set_min_width(HOTBAR_WIDTH);
        let settings_height = 190.0;
        let hotbar_height = (ui.available_height() - settings_height).max(160.0);
        ui.allocate_ui(egui::vec2(ui.available_width(), hotbar_height), |ui| {
            editor.show_hotbar(ui);
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("logic-tool-settings")
            .max_height(settings_height)
            .show(ui, |ui| {
                editor.show_tool_settings(ui);
            });
    }
}
