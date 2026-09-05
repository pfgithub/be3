use std::sync::Arc;

use block::Block as _;
use block_client::blocks::deterministic_game::{DeterministicGame, DeterministicGameOperation};
use block_client::blocks::game_module::GameModule;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, BlockFilter, BlockPicker, EditorHost};
use game_api::{GameAction, GameScreen};
use game_host::Game;
use uuid::Uuid;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(360.0, 320.0);

struct Loaded {
    module: Uuid,
    revision: u64,
    game: Result<Arc<Game>, String>,
}

#[derive(Default)]
pub struct DeterministicGameApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<DeterministicGame>>,
    module: Option<BlockHandle<GameModule>>,
    player: Uuid,
    picker: BlockPicker,
    chosen: Option<Uuid>,
    loaded: Option<Loaded>,
    shown: Vec<GameAction>,
    screen: Option<Result<GameScreen, String>>,
}

impl DeterministicGameApp {
    fn module(&mut self, module: Uuid) -> &BlockHandle<GameModule> {
        let client = self.client.as_ref().expect("the editor is connected");
        if self
            .module
            .as_ref()
            .is_none_or(|handle| handle.id() != module)
        {
            self.module = Some(client.get_block::<GameModule>(module));
        }
        self.module.as_ref().expect("the handle was just taken")
    }

    fn game(&mut self, module: Uuid) -> Option<Result<Arc<Game>, String>> {
        let revision = self.module(module).revision();
        if let Some(loaded) = &self.loaded {
            if loaded.module == module && loaded.revision == revision {
                return Some(loaded.game.clone());
            }
        }
        let bytes = self.module(module).read()?.data().to_vec();
        let game = Game::load(&bytes).map(Arc::new);
        self.loaded = Some(Loaded {
            module,
            revision,
            game: game.clone(),
        });
        Some(game)
    }

    fn screen(&mut self, game: &Game, actions: Vec<GameAction>) -> &Result<GameScreen, String> {
        if self.screen.is_none() || self.shown != actions {
            let screen = game.show(&actions, self.player);
            self.shown = actions;
            self.screen = Some(screen);
        }
        self.screen.as_ref().expect("the screen was just computed")
    }

    fn chosen_name(&self) -> Option<String> {
        let chosen = self.chosen?;
        let handle = self.client.as_ref()?.get_block::<GameModule>(chosen);
        let name = match handle.read() {
            Some(module) => module.source_name().to_owned(),
            None => "Loading…".to_owned(),
        };
        Some(name)
    }
}

impl block_editor_plugin::App for DeterministicGameApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.player = client.account_id();
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        self.client = Some(client);
        self.host = Some(host);
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        let Some(host) = self.host.clone() else {
            return;
        };
        match self.picker.poll(&host) {
            Some(Ok((module, _))) => self.chosen = Some(module),
            Some(Err(error)) => {
                ui.colored_label(ui.visuals().error_fg_color, error)
                    .test_id("game.error");
            }
            None => {}
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.picker.is_open(),
                    egui::Button::new("Choose game module..."),
                )
                .test_id("game.choose")
                .clicked()
            {
                self.picker.open(&host, module_filter());
            }
            match self.chosen_name() {
                Some(name) => ui.label(name),
                None => ui.weak("No game module chosen"),
            };
        });
        host.set_creation_ready(self.chosen.is_some());
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .client
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        let module = self.chosen.ok_or("Choose a game module first")?;
        let block = client.create_block(DeterministicGame::new(module));
        Ok(block.id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(state) = self.block.as_ref().and_then(|block| block.read()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let module = state.module();
        let actions = state.actions().to_vec();
        drop(state);

        let game = match self.game(module) {
            Some(Ok(game)) => game,
            Some(Err(error)) => {
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error)
                        .test_id("game.error");
                });
                return;
            }
            None => {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
                return;
            }
        };

        let screen = match self.screen(&game, actions) {
            Ok(screen) => screen.clone(),
            Err(error) => {
                let error = error.clone();
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error)
                        .test_id("game.error");
                });
                return;
            }
        };

        let editable = self.host.as_ref().is_none_or(EditorHost::editable);
        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(screen.description);
            ui.add_space(8.0);
            for (index, option) in screen.actions.into_iter().enumerate() {
                if ui
                    .add_enabled(editable, egui::Button::new(option.label))
                    .test_id(&format!("game.action.{index}"))
                    .clicked()
                {
                    if let Some(block) = self.block.as_ref() {
                        block.operate(DeterministicGameOperation::Append {
                            action: option.effect,
                        });
                    }
                }
            }
        });
    }
}

pub(crate) fn module_filter() -> BlockFilter {
    BlockFilter {
        name: "Game module".to_owned(),
        block_types: vec![GameModule::TYPE_ID.into_bytes()],
        templates: false,
    }
}
