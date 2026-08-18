use block_client::{
    blocks::deterministic_game::{DeterministicGame, DeterministicGameOperation},
    BlockClient, BlockHandle,
};
use deterministic_games::{tic_tac_toe::TicTacToe, Game};
use eframe::egui;
use egui_material_icons::{icons::ICON_GRID_3X3, MaterialIcon};
use uuid::Uuid;

use super::{
    BlockEditor, CreatableEditor, DirectEditorCapabilities, DirectEditorViewport, EditorAccess,
    EditorAction, EditorKind,
};

const TIC_TAC_TOE: &str = "tic_tac_toe";

/// The only place a game identifier is turned into its `Game` implementation.
/// Every `DeterministicGame` block shares one editor, so a block created by a
/// future game this build does not recognize falls back to a plain message
/// rather than failing to open.
fn game_for(kind: &str) -> Option<&'static dyn Game> {
    match kind {
        TIC_TAC_TOE => Some(&TicTacToe),
        _ => None,
    }
}

impl EditorKind for DeterministicGameEditor {
    type Block = DeterministicGame;

    const DISPLAY_NAME: &'static str = "Tic-Tac-Toe";
    const ICON: MaterialIcon = ICON_GRID_3X3;

    fn open(client: &BlockClient, block: BlockHandle<DeterministicGame>) -> Self {
        Self::new(client, block)
    }
}

impl CreatableEditor for DeterministicGameEditor {
    fn create(client: &BlockClient) -> Self {
        let block = client.create_block(DeterministicGame::new(TIC_TAC_TOE));
        Self::new(client, block)
    }
}

pub(super) struct DeterministicGameEditor {
    block: BlockHandle<DeterministicGame>,
    player: Uuid,
}

impl DeterministicGameEditor {
    fn new(client: &BlockClient, block: BlockHandle<DeterministicGame>) -> Self {
        Self {
            block,
            player: client.account_id(),
        }
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
        let kind = block.game().to_owned();
        let actions = block.actions().to_vec();
        drop(block);

        let Some(game) = game_for(&kind) else {
            ui.label(format!("Unsupported game: {kind}"));
            return None;
        };
        let screen = game.show(&actions, self.player);
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
