use block_client::{
    blocks::deterministic_game::{
        DeterministicGame, DeterministicGameKind, DeterministicGameOperation,
    },
    BlockClient, BlockHandle,
};
use deterministic_games::{crazy_8s::Crazy8s, tic_tac_toe::TicTacToe, Game};
use eframe::egui;
use egui_material_icons::{icons::ICON_GRID_3X3, MaterialIcon};
use uuid::Uuid;

use super::{
    BlockEditor, ConfigurableEditor, CreationOptions, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

const GAME_KINDS: [DeterministicGameKind; 2] = [
    DeterministicGameKind::TicTacToe,
    DeterministicGameKind::Crazy8s,
];

/// The only place a game kind is turned into its `Game` implementation.
fn game_for(kind: DeterministicGameKind) -> &'static dyn Game {
    match kind {
        DeterministicGameKind::TicTacToe => &TicTacToe,
        DeterministicGameKind::Crazy8s => &Crazy8s,
    }
}

impl EditorKind for DeterministicGameEditor {
    type Block = DeterministicGame;

    const DISPLAY_NAME: &'static str = "Game";
    const ICON: MaterialIcon = ICON_GRID_3X3;

    fn open(client: &BlockClient, block: BlockHandle<DeterministicGame>) -> Self {
        Self::new(client, block)
    }
}

/// Which game a `DeterministicGame` block plays is fixed for its lifetime
/// (there is no operation to change it), so creating one starts by choosing
/// it.
impl ConfigurableEditor for DeterministicGameEditor {
    type Options = ChosenGame;

    fn create(client: &BlockClient, options: ChosenGame) -> Result<Self, String> {
        let kind = options.kind.ok_or("Choose a game first")?;
        let block = client.create_block(DeterministicGame::new(kind));
        Ok(Self::new(client, block))
    }
}

#[derive(Default)]
pub(crate) struct ChosenGame {
    kind: Option<DeterministicGameKind>,
}

impl CreationOptions for ChosenGame {
    fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label("Choose a game:");
        for kind in GAME_KINDS {
            ui.radio_value(&mut self.kind, Some(kind), kind.display_name());
        }
        self.kind.is_some()
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
        let kind = block.game();
        let actions = block.actions().to_vec();
        drop(block);

        let screen = game_for(kind).show(&actions, self.player);
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
