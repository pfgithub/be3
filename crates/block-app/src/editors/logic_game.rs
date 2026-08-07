mod binary_addition;

use std::collections::HashMap;

use block::{Block, BlockParent};
use block_client::{
    blocks::{
        hotbar::Hotbar,
        logic_game::{LogicGame, LogicGameOperation},
        logic_grid::LogicGrid,
    },
    BlockClient, BlockHandle,
};
use eframe::egui;
use egui_material_icons::{
    icons::{ICON_ADD, ICON_CHECK_CIRCLE, ICON_DELETE, ICON_SPORTS_ESPORTS, ICON_WIDGETS},
    MaterialIcon,
};
use logicgame::challenges::{generate_challenge, ChallengeId};
use uuid::Uuid;

use self::binary_addition::BinaryAdditionQuiz;
use super::{
    settings::RootSetting, BlockEditor, CreatableEditor, DirectEditorCapabilities,
    DirectEditorViewport, EditorAccess, EditorAction, EditorKind,
};

const DIRECT_EDITOR_WIDTH: f32 = 720.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 26.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 120.0;

pub(super) struct LogicGameEditor {
    block: BlockHandle<LogicGame>,
    /// Every solution grid the game lists, so each one can be named and its
    /// progress read back without opening it.
    solutions: HashMap<Uuid, BlockHandle<LogicGrid>>,
    /// The level whose solutions are shown, if any.
    expanded: Option<ChallengeId>,
    /// The palette the game's grids are built from. The game does not own it:
    /// it is the hotbar at the root of the block tree, offered here so it can
    /// be opened without finding a grid first.
    hotbar: Option<RootSetting<Hotbar>>,
    quiz: BinaryAdditionQuiz,
}

impl EditorKind for LogicGameEditor {
    type Block = LogicGame;

    const DISPLAY_NAME: &'static str = "Logic Game";
    const ICON: MaterialIcon = ICON_SPORTS_ESPORTS;

    fn open(_client: &BlockClient, block: BlockHandle<LogicGame>) -> Self {
        Self::new(block)
    }
}

impl CreatableEditor for LogicGameEditor {
    fn create(client: &BlockClient) -> Self {
        Self::new(client.create_block(LogicGame::new()))
    }
}

impl LogicGameEditor {
    fn new(block: BlockHandle<LogicGame>) -> Self {
        Self {
            block,
            solutions: HashMap::new(),
            expanded: None,
            hotbar: None,
            quiz: BinaryAdditionQuiz::default(),
        }
    }

    fn start_solution(
        &mut self,
        client: &BlockClient,
        challenge: ChallengeId,
        index: usize,
    ) -> EditorAction {
        let solution = client.create_block(LogicGrid::for_challenge(challenge));
        solution.set_name(format!("{} {}", challenge.name(), index + 1));
        solution.set_parent(BlockParent::Uuid(self.block.id()));
        let id = solution.id();
        self.solutions.insert(id, solution);
        self.block.operate(LogicGameOperation::InsertSolution {
            challenge,
            solution: id,
            index,
        });
        EditorAction::OpenBlock {
            id,
            block_type: LogicGrid::TYPE_ID,
        }
    }

    /// Keeps a handle on every listed solution and carries each level's
    /// progress over from the grids that passed it, and looks for the shared
    /// hotbar so it can be offered once the block tree has one.
    fn sync(&mut self, client: &BlockClient, client_id: Uuid) {
        self.hotbar
            .get_or_insert_with(|| RootSetting::new(client))
            .find(client, client_id);
        let Some(game) = self.block.read() else {
            return;
        };
        let levels = game
            .levels()
            .iter()
            .map(|level| (level.challenge, level.solutions.clone(), level.completed))
            .collect::<Vec<_>>();
        drop(game);

        let listed = levels
            .iter()
            .flat_map(|(_, solutions, _)| solutions.iter().copied())
            .collect::<Vec<_>>();
        self.solutions.retain(|id, _| listed.contains(id));
        for solution in &listed {
            self.solutions
                .entry(*solution)
                .or_insert_with(|| client.get_block::<LogicGrid>(*solution));
        }

        for (challenge, solutions, completed) in levels {
            let passed = solutions.iter().any(|solution| {
                self.solutions
                    .get(solution)
                    .and_then(BlockHandle::read)
                    .is_some_and(|grid| grid.completed())
            });
            if passed && !completed {
                self.block.operate(LogicGameOperation::SetCompleted {
                    challenge,
                    completed: true,
                });
            }
        }
    }
}

impl BlockEditor for LogicGameEditor {
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
        let game = self.block.read()?;
        let rows = game.levels().len()
            + game
                .levels()
                .iter()
                .filter(|level| Some(level.challenge) == self.expanded)
                .map(|level| level.solutions.len() + 1)
                .sum::<usize>();
        drop(game);
        let quiz = if self.expanded == Some(ChallengeId::BinaryAddition) {
            self.quiz.height()
        } else {
            0.0
        };
        Some(egui::vec2(
            DIRECT_EDITOR_WIDTH,
            DIRECT_EDITOR_CHROME_HEIGHT + DIRECT_EDITOR_ROW_HEIGHT * rows as f32 + quiz,
        ))
    }

    fn direct_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        editors: &mut EditorAccess<'_>,
        _scale: f32,
        _viewport: &mut DirectEditorViewport,
    ) -> Option<EditorAction> {
        let client = editors.client();
        self.sync(client, editors.client_id());
        let Some(game) = self.block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return None;
        };
        let levels = game
            .levels()
            .iter()
            .map(|level| (level.challenge, level.solutions.clone(), level.completed))
            .collect::<Vec<_>>();
        drop(game);
        let hotbar = self
            .hotbar
            .as_ref()
            .and_then(RootSetting::block)
            .map(BlockHandle::id);

        let mut action = None;
        ui.horizontal(|ui| {
            ui.heading("Levels");
            if let Some(hotbar) = hotbar {
                if ui
                    .button(format!("{} Hotbar", ICON_WIDGETS.codepoint))
                    .clicked()
                {
                    action = Some(EditorAction::OpenBlock {
                        id: hotbar,
                        block_type: Hotbar::TYPE_ID,
                    });
                }
            }
        });
        ui.add_space(8.0);

        let mut remove = None;
        let mut start = None;
        for (challenge, solutions, completed) in &levels {
            let expanded = self.expanded == Some(*challenge);
            ui.horizontal(|ui| {
                if ui.selectable_label(expanded, challenge.name()).clicked() {
                    self.expanded = (!expanded).then_some(*challenge);
                }
                if *completed {
                    ui.colored_label(
                        egui::Color32::from_rgb(115, 209, 133),
                        ICON_CHECK_CIRCLE.codepoint,
                    )
                    .on_hover_text("Passed");
                }
                ui.weak(format!("{} attempts", solutions.len()));
            });
            if !expanded {
                continue;
            }
            ui.indent(("level", *challenge as usize), |ui| {
                ui.weak(generate_challenge(*challenge).goal);
                if *challenge == ChallengeId::BinaryAddition {
                    // The quiz level has nothing to wire up, so it is answered
                    // here rather than in a grid.
                    self.quiz.ui(ui, &self.block);
                    return;
                }
                for solution in solutions {
                    ui.horizontal(|ui| {
                        let name = self.solutions.get(solution).map_or_else(
                            || "Loading…".to_owned(),
                            |handle| {
                                super::display_name(
                                    editors.registry(),
                                    LogicGrid::TYPE_ID,
                                    handle.name().as_deref(),
                                )
                            },
                        );
                        if ui.link(name).clicked() {
                            action = Some(EditorAction::OpenBlock {
                                id: *solution,
                                block_type: LogicGrid::TYPE_ID,
                            });
                        }
                        if self
                            .solutions
                            .get(solution)
                            .and_then(BlockHandle::read)
                            .is_some_and(|grid| grid.completed())
                        {
                            ui.colored_label(
                                egui::Color32::from_rgb(115, 209, 133),
                                ICON_CHECK_CIRCLE.codepoint,
                            );
                        }
                        if ui
                            .small_button(ICON_DELETE)
                            .on_hover_text("Remove from this level")
                            .clicked()
                        {
                            remove = Some((*challenge, *solution));
                        }
                    });
                }
                if ui
                    .button(format!("{} New attempt", ICON_ADD.codepoint))
                    .clicked()
                {
                    start = Some((*challenge, solutions.len()));
                }
            });
        }

        if let Some((challenge, solution)) = remove {
            self.block.operate(LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            });
        }
        if let Some((challenge, index)) = start {
            action = Some(self.start_solution(client, challenge, index));
        }
        action
    }
}
