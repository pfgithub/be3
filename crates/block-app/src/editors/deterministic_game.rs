use block_client::{
    blocks::deterministic_game::{DeterministicGame, DeterministicGameOperation},
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{icons::ICON_GRID_3X3, MaterialIcon};
use game_host::{GameAction, GameScreen};
use uuid::Uuid;

use super::{
    BlockEditor, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

pub(crate) mod catalog;

impl EditorKind for DeterministicGameEditor {
    type Block = DeterministicGame;

    const DISPLAY_NAME: &'static str = "Game";
    const ICON: MaterialIcon = ICON_GRID_3X3;

    fn open(client: &BlockClient, block: BlockHandle<DeterministicGame>) -> Self {
        Self::new(client, block)
    }
}

impl ConfigurableEditor for DeterministicGameEditor {
    type Options = ChosenGame;

    fn create(client: &BlockClient, options: ChosenGame) -> Result<Self, String> {
        let id = options.id.ok_or("Choose a game first")?;
        let installed = catalog::installed();
        let entry = installed
            .games()
            .iter()
            .find(|entry| entry.id() == id)
            .ok_or("That game is no longer installed")?;
        let block = client.create_block(DeterministicGame::new(
            entry.id().to_owned(),
            entry.display_name().to_owned(),
        ));
        Ok(Self::new(client, block))
    }
}

#[derive(Default)]
pub(crate) struct ChosenGame {
    id: Option<String>,
}

impl CreationOptions for ChosenGame {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let installed = catalog::installed();
        if installed.games().is_empty() {
            ui.label("No games are installed.");
        } else {
            ui.label("Choose a game:");
            for entry in installed.games() {
                let chosen = self.id.as_deref() == Some(entry.id());
                if ui.radio(chosen, entry.display_name()).clicked() {
                    self.id = Some(entry.id().to_owned());
                }
            }
        }
        for error in installed.errors() {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        self.id.is_some()
    }
}

pub(super) struct DeterministicGameEditor {
    block: BlockHandle<DeterministicGame>,
    player: Uuid,
    shown: Vec<GameAction>,
    screen: Option<Result<GameScreen, String>>,
}

impl DeterministicGameEditor {
    fn new(client: &BlockClient, block: BlockHandle<DeterministicGame>) -> Self {
        Self {
            block,
            player: client.account_id(),
            shown: Vec::new(),
            screen: None,
        }
    }

    fn screen(&mut self, game: &str, actions: Vec<GameAction>) -> &Result<GameScreen, String> {
        if self.screen.is_none() || self.shown != actions {
            let screen = match catalog::game(game) {
                Some(module) => module.show(&actions, self.player),
                None => Err(format!("{game} is not installed")),
            };
            self.shown = actions;
            self.screen = Some(screen);
        }
        self.screen.as_ref().expect("the screen was just computed")
    }
}

impl BlockEditor for DeterministicGameEditor {
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
        Some(egui::vec2(360.0, 320.0))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let Some(block) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let game = block.game().to_owned();
        let actions = block.actions().to_vec();
        drop(block);

        let screen = match self.screen(&game, actions) {
            Ok(screen) => screen.clone(),
            Err(error) => {
                let error = error.clone();
                ui.vertical_centered(|ui| {
                    ui.add_space(12.0);
                    ui.colored_label(ui.visuals().error_fg_color, error);
                });
                return None;
            }
        };

        ui.vertical_centered(|ui| {
            ui.add_space(12.0);
            ui.label(screen.description);
            ui.add_space(8.0);
            for option in screen.actions {
                if ui.button(option.label).clicked() {
                    self.block.operate(DeterministicGameOperation::Append {
                        action: option.effect,
                    });
                }
            }
        });
        None
    }
}
