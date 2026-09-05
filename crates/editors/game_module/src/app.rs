use std::sync::Arc;

use block_client::blocks::game_module::{GameModule, GameModuleOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, EditorHost, FileFilter, FilePicker, PickedFile};
use game_host::Game;
use uuid::Uuid;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(320.0, 120.0);

struct Editing {
    host: EditorHost,
    _client: Arc<BlockClient>,
    block: BlockHandle<GameModule>,
}

struct Creating {
    host: EditorHost,
    client: Arc<BlockClient>,
    chosen: Option<GameModule>,
}

#[derive(Default)]
struct Loaded {
    revision: Option<u64>,
    name: Option<String>,
    error: Option<String>,
}

#[derive(Default)]
pub struct GameModuleApp {
    editing: Option<Editing>,
    creation: Option<Creating>,
    picker: FilePicker,
    error: Option<String>,
    loaded: Loaded,
}

impl block_editor_plugin::App for GameModuleApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        let block = client.get_block::<GameModule>(block_id);
        self.editing = Some(Editing {
            host,
            _client: client,
            block,
        });
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(Creating {
            host,
            client,
            chosen: None,
        });
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let creation = self
            .creation
            .as_mut()
            .ok_or("this editor is not filling in a block")?;
        let module = creation.chosen.take().ok_or("no file was chosen")?;
        Ok(creation.client.create_block(module).id())
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        let Some(Creating { host, chosen, .. }) = &mut self.creation else {
            return;
        };
        match self.picker.poll(host).map(|file| file.and_then(imported)) {
            Some(Ok(module)) => {
                host.set_creation_ready(true);
                *chosen = Some(module);
                self.error = None;
            }
            Some(Err(error)) => {
                host.set_creation_ready(false);
                *chosen = None;
                self.error = Some(error);
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(!self.picker.is_open(), egui::Button::new("Choose file..."))
                .test_id("game-module.choose")
                .clicked()
            {
                self.picker.open(host, filter());
            }
            match chosen {
                Some(module) => ui.label(module.source_name()),
                None => ui.weak("No file chosen"),
            };
        });
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error)
                .test_id("game-module.error");
        }
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(module) = self
            .editing
            .as_ref()
            .and_then(|editing| editing.block.read())
        else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let source_name = module.source_name().to_owned();
        let bytes = module.data().len();
        drop(module);
        self.load();
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            match (&self.loaded.name, &self.loaded.error) {
                (Some(name), _) => {
                    ui.heading(name).test_id("game-module.name");
                }
                (None, Some(error)) => {
                    ui.colored_label(ui.visuals().error_fg_color, error)
                        .test_id("game-module.error");
                }
                (None, None) => {
                    ui.spinner();
                }
            }
            ui.label(source_name);
            ui.weak(format!("{bytes} bytes"));
        });
    }

    fn right_sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Game module");
        let Some(editing) = &self.editing else {
            return;
        };
        match self
            .picker
            .poll(&editing.host)
            .map(|file| file.and_then(imported))
        {
            Some(Ok(module)) => {
                editing
                    .block
                    .operate(GameModuleOperation::Replace { module });
                self.error = None;
            }
            Some(Err(error)) => self.error = Some(error),
            None => {}
        }
        if ui
            .add_enabled(
                !self.picker.is_open() && editing.host.editable(),
                egui::Button::new("Replace module..."),
            )
            .test_id("game-module.replace")
            .clicked()
        {
            self.picker.open(&editing.host, filter());
        }
        if let Some(error) = &self.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
    }
}

impl GameModuleApp {
    fn load(&mut self) {
        let Some(editing) = &self.editing else {
            return;
        };
        let revision = editing.block.revision();
        if self.loaded.revision == Some(revision) {
            return;
        }
        let Some(module) = editing.block.read() else {
            return;
        };
        let loaded = match Game::load(module.data()) {
            Ok(game) => Loaded {
                revision: Some(revision),
                name: Some(game.name().to_owned()),
                error: None,
            },
            Err(error) => Loaded {
                revision: Some(revision),
                name: None,
                error: Some(error),
            },
        };
        self.loaded = loaded;
    }
}

fn filter() -> FileFilter {
    FileFilter {
        name: "Game modules".to_owned(),
        default_file_name: "Game".to_owned(),
        extensions: GameModule::FILE_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect(),
        mime_types: GameModule::MIME_TYPES
            .iter()
            .map(|mime_type| (*mime_type).to_owned())
            .collect(),
    }
}

pub(crate) fn imported(file: PickedFile) -> Result<GameModule, String> {
    let PickedFile { name, data } = file;
    Game::load(&data).map_err(|error| format!("Could not import {name}: {error}"))?;
    Ok(GameModule::new(name, data))
}
