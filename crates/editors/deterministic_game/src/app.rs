use std::sync::Arc;

use block_client::blocks::deterministic_game::{DeterministicGame, DeterministicGameOperation};
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::{egui, EditorHost};
use game_api::{GameAction, GameScreen};
use uuid::Uuid;

use crate::catalog::Catalog;

const INTRINSIC_SIZE: egui::Vec2 = egui::vec2(360.0, 320.0);

#[derive(Default)]
pub struct DeterministicGameApp {
    host: Option<EditorHost>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<DeterministicGame>>,
    player: Uuid,
    catalog: Catalog,
    chosen: Option<String>,
    shown: Vec<GameAction>,
    screen: Option<Result<GameScreen, String>>,
}

impl DeterministicGameApp {
    fn poll_catalog(&mut self) {
        let Some(host) = self.host.as_ref() else {
            return;
        };
        if !self.catalog.installed() {
            self.catalog.poll(host);
        }
    }

    fn screen(&mut self, game: &str, actions: Vec<GameAction>) -> &Result<GameScreen, String> {
        if self.screen.is_none() || self.shown != actions {
            let screen = match self.catalog.game(game) {
                Some(module) => module.show(&actions, self.player),
                None => Err(format!("{game} is not installed")),
            };
            self.shown = actions;
            self.screen = Some(screen);
        }
        self.screen.as_ref().expect("the screen was just computed")
    }
}

impl block_editor_plugin::App for DeterministicGameApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.player = client.account_id();
        self.block = Some(client.get_block(block_id));
        self.host = Some(host);
    }

    fn connect_creation(&mut self, host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
        self.host = Some(host);
    }

    fn creation_ui(&mut self, ui: &mut egui::Ui) {
        self.poll_catalog();
        if !self.catalog.installed() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Looking for installed games…");
            });
            return;
        }
        if self.catalog.games().is_empty() {
            ui.label("No games are installed.");
        } else {
            ui.label("Choose a game:");
            for entry in self.catalog.games() {
                let chosen = self.chosen.as_deref() == Some(entry.id());
                if ui
                    .radio(chosen, entry.display_name())
                    .test_id(&format!("game.choose.{}", entry.id()))
                    .clicked()
                {
                    self.chosen = Some(entry.id().to_owned());
                }
            }
        }
        for error in self.catalog.errors() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if let Some(host) = self.host.as_ref() {
            host.set_creation_ready(self.chosen.is_some());
        }
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        let id = self.chosen.as_deref().ok_or("Choose a game first")?;
        let entry = self
            .catalog
            .games()
            .iter()
            .find(|entry| entry.id() == id)
            .ok_or("That game is no longer installed")?;
        let block = client.create_block(DeterministicGame::new(
            entry.id().to_owned(),
            entry.display_name().to_owned(),
        ));
        Ok(block.id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        Some(INTRINSIC_SIZE)
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.poll_catalog();
        let Some(state) = self.block.as_ref().and_then(|block| block.read()) else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let game = state.game().to_owned();
        let actions = state.actions().to_vec();
        drop(state);
        if !self.catalog.installed() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

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
