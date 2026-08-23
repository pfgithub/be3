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
use block_client::references::{ReferenceClassificationQueue, ReferenceResolutionCache};

const DIRECT_EDITOR_WIDTH: f32 = 720.0;
const DIRECT_EDITOR_ROW_HEIGHT: f32 = 26.0;
const DIRECT_EDITOR_CHROME_HEIGHT: f32 = 120.0;

pub(super) struct LogicGameEditor {
    block: BlockHandle<LogicGame>,

    solutions: HashMap<Uuid, BlockHandle<LogicGrid>>,

    expanded: Option<ChallengeId>,

    hotbar: Option<RootSetting<Hotbar>>,
    quiz: BinaryAdditionQuiz,
    reference_cache: ReferenceResolutionCache,
    pending_solutions: ReferenceClassificationQueue<(ChallengeId, usize)>,
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
            reference_cache: ReferenceResolutionCache::default(),
            pending_solutions: ReferenceClassificationQueue::default(),
        }
    }

    fn start_solution(
        &mut self,
        editors: &mut EditorAccess<'_>,
        challenge: ChallengeId,
        index: usize,
    ) -> EditorAction {
        let solution = editors
            .client()
            .create_block(LogicGrid::for_challenge(challenge));
        solution.set_name(format!("{} {}", challenge.name(), index + 1));
        solution.set_parent(BlockParent::Uuid(self.block.id()));
        let id = solution.id();
        self.solutions.insert(id, solution);
        let client = editors.client_handle();
        self.pending_solutions
            .push(&client, self.block.id(), id, (challenge, index));
        EditorAction::OpenBlock {
            id,
            block_type: LogicGrid::TYPE_ID,
        }
    }

    fn sync(&mut self, editors: &mut EditorAccess<'_>) {
        self.reference_cache.poll();
        for (solution, (challenge, index)) in self.pending_solutions.poll() {
            self.block.operate(LogicGameOperation::InsertSolution {
                challenge,
                solution,
                index,
            });
        }
        self.hotbar
            .get_or_insert_with(|| RootSetting::new(editors.client()))
            .find(editors.client(), editors.client_id());
        let Some(game) = self.block.read() else {
            return;
        };
        let levels = game
            .levels()
            .iter()
            .map(|level| (level.challenge, level.solutions.clone(), level.completed))
            .collect::<Vec<_>>();
        drop(game);

        let referencing_id = self.block.id();
        let client = editors.client_handle();
        let mut listed = Vec::new();
        for (_, solutions, _) in &levels {
            for solution in solutions {
                if let Some(id) = self
                    .reference_cache
                    .resolve(&client, referencing_id, *solution)
                {
                    listed.push(id);
                }
            }
        }
        self.solutions.retain(|id, _| listed.contains(id));
        for solution in &listed {
            self.solutions
                .entry(*solution)
                .or_insert_with(|| editors.client().get_block::<LogicGrid>(*solution));
        }

        for (challenge, solutions, completed) in levels {
            let passed = solutions.iter().any(|solution| {
                self.reference_cache
                    .resolve(&client, referencing_id, *solution)
                    .and_then(|id| self.solutions.get(&id))
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
        self.sync(editors);
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

        let referencing_id = self.block.id();
        let client = editors.client_handle();
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
                    self.quiz.ui(ui, &self.block);
                    return;
                }
                for solution in solutions {
                    ui.horizontal(|ui| {
                        let resolved_id =
                            self.reference_cache
                                .resolve(&client, referencing_id, *solution);
                        let label =
                            resolved_id
                                .and_then(|id| self.solutions.get(&id))
                                .map(|handle| {
                                    super::BlockLabel::for_handle(editors.registry(), handle)
                                });
                        let name = label.as_ref().map_or_else(
                            || {
                                egui::RichText::new(if resolved_id.is_some() {
                                    "Loading…"
                                } else {
                                    "Broken link"
                                })
                            },
                            super::BlockLabel::rich_text,
                        );
                        if let Some(id) = resolved_id {
                            if ui.link(name).clicked() {
                                action = Some(EditorAction::OpenBlock {
                                    id,
                                    block_type: LogicGrid::TYPE_ID,
                                });
                            }
                        } else {
                            ui.weak(name);
                        }
                        if resolved_id
                            .and_then(|id| self.solutions.get(&id))
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
            action = Some(self.start_solution(editors, challenge, index));
        }
        action
    }
}
