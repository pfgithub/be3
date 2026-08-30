use std::collections::HashMap;
use std::sync::Arc;

use block::{Block, BlockParent};
use block_client::blocks::hotbar::Hotbar;
use block_client::blocks::logic_game::{LogicGame, LogicGameOperation};
use block_client::blocks::logic_grid::LogicGrid;
use block_client::references::{ReferenceClassificationQueue, ReferenceResolutionCache};
use block_client::root_settings::RootSetting;
use block_client::{BlockClient, BlockHandle};
use block_editor_plugin::block_ui::test_id::TestId;
use block_editor_plugin::block_ui::BlockLabel;
use block_editor_plugin::egui_material_icons::icons::{
    ICON_ADD, ICON_CHECK_CIRCLE, ICON_DELETE, ICON_WIDGETS,
};
use block_editor_plugin::{egui, EditorHost};
use logicgame::challenges::{generate_challenge, ChallengeId};
use uuid::Uuid;

use crate::binary_addition::BinaryAdditionQuiz;

const INTRINSIC_WIDTH: f32 = 720.0;
const ROW_HEIGHT: f32 = 26.0;
const CHROME_HEIGHT: f32 = 120.0;
const PASSED: egui::Color32 = egui::Color32::from_rgb(115, 209, 133);

#[derive(Default)]
pub struct LogicGameApp {
    host: Option<EditorHost>,
    client: Option<Arc<BlockClient>>,
    creation: Option<Arc<BlockClient>>,
    block: Option<BlockHandle<LogicGame>>,
    solutions: HashMap<Uuid, BlockHandle<LogicGrid>>,
    expanded: Option<ChallengeId>,
    hotbar: Option<RootSetting<Hotbar>>,
    quiz: BinaryAdditionQuiz,
    reference_cache: ReferenceResolutionCache,
    pending_solutions: ReferenceClassificationQueue<(ChallengeId, usize)>,
}

impl LogicGameApp {
    fn start_solution(&mut self, challenge: ChallengeId, index: usize) {
        let (Some(host), Some(client), Some(block)) = (
            self.host.as_ref(),
            self.client.as_ref(),
            self.block.as_ref(),
        ) else {
            return;
        };
        let solution = client.create_block(LogicGrid::for_challenge(challenge));
        solution.set_name(format!("{} {}", challenge.name(), index + 1));
        solution.set_parent(BlockParent::Uuid(block.id()));
        let id = solution.id();
        self.solutions.insert(id, solution);
        self.pending_solutions
            .push(client, block.id(), id, (challenge, index));
        host.open_block(id, LogicGrid::TYPE_ID);
    }

    fn sync(&mut self) {
        let (Some(host), Some(client), Some(block)) =
            (self.host.as_ref(), self.client.clone(), self.block.clone())
        else {
            return;
        };
        self.reference_cache.poll();
        for (solution, (challenge, index)) in self.pending_solutions.poll() {
            block.operate(LogicGameOperation::InsertSolution {
                challenge,
                solution,
                index,
            });
        }
        self.hotbar
            .get_or_insert_with(|| RootSetting::new(&client))
            .find(&client, host.client_id());
        let Some(game) = block.read() else {
            return;
        };
        let levels = game
            .levels()
            .iter()
            .map(|level| (level.challenge, level.solutions.clone(), level.completed))
            .collect::<Vec<_>>();
        drop(game);

        let referencing_id = block.id();
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
                .or_insert_with(|| client.get_block::<LogicGrid>(*solution));
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
                block.operate(LogicGameOperation::SetCompleted {
                    challenge,
                    completed: true,
                });
            }
        }
    }
}

impl block_editor_plugin::App for LogicGameApp {
    fn connect(&mut self, host: EditorHost, client: Arc<BlockClient>, block_id: Uuid) {
        self.block = Some(client.get_block(block_id));
        self.client = Some(client);
        self.host = Some(host);
    }

    fn connect_creation(&mut self, _host: EditorHost, client: Arc<BlockClient>) {
        self.creation = Some(client);
    }

    fn create_block(&mut self) -> Result<Uuid, String> {
        let client = self
            .creation
            .as_ref()
            .ok_or("this editor is not creating a block")?;
        Ok(client.create_block(LogicGame::new()).id())
    }

    fn intrinsic_size(&mut self) -> Option<egui::Vec2> {
        let game = self.block.as_ref()?.read()?;
        let rows = game.levels().len()
            + game
                .levels()
                .iter()
                .filter(|level| Some(level.challenge) == self.expanded)
                .map(|level| level.solutions.len() + 1)
                .sum::<usize>();
        drop(game);
        let quiz = match self.expanded == Some(ChallengeId::BinaryAddition) {
            true => self.quiz.height(),
            false => 0.0,
        };
        Some(egui::vec2(
            INTRINSIC_WIDTH,
            CHROME_HEIGHT + ROW_HEIGHT * rows as f32 + quiz,
        ))
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync();
        let (Some(host), Some(client), Some(block)) =
            (self.host.clone(), self.client.clone(), self.block.clone())
        else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let Some(game) = block.read() else {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        };
        let levels = game
            .levels()
            .iter()
            .map(|level| (level.challenge, level.solutions.clone(), level.completed))
            .collect::<Vec<_>>();
        drop(game);
        let editable = host.editable();
        let types = host.block_types();
        let hotbar = self
            .hotbar
            .as_ref()
            .and_then(RootSetting::block)
            .map(BlockHandle::id);

        ui.horizontal(|ui| {
            ui.heading("Levels");
            if let Some(hotbar) = hotbar {
                if ui
                    .button(format!("{} Hotbar", ICON_WIDGETS.codepoint))
                    .test_id("logic-game.hotbar")
                    .clicked()
                {
                    host.open_block(hotbar, Hotbar::TYPE_ID);
                }
            }
        });
        ui.add_space(8.0);

        let referencing_id = block.id();
        let mut remove = None;
        let mut start = None;
        for (challenge, solutions, completed) in &levels {
            let expanded = self.expanded == Some(*challenge);
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(expanded, challenge.name())
                    .test_id(&format!("logic-game.level.{}", *challenge as usize))
                    .clicked()
                {
                    self.expanded = (!expanded).then_some(*challenge);
                }
                if *completed {
                    ui.colored_label(PASSED, ICON_CHECK_CIRCLE.codepoint)
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
                    self.quiz.ui(ui, &block, editable);
                    return;
                }
                for solution in solutions {
                    ui.horizontal(|ui| {
                        let resolved_id =
                            self.reference_cache
                                .resolve(&client, referencing_id, *solution);
                        let label = resolved_id
                            .and_then(|id| self.solutions.get(&id))
                            .map(|handle| BlockLabel::for_handle(types.as_ref(), handle));
                        let name = label.as_ref().map_or_else(
                            || {
                                egui::RichText::new(match resolved_id.is_some() {
                                    true => "Loading…",
                                    false => "Broken link",
                                })
                            },
                            BlockLabel::rich_text,
                        );
                        if let Some(id) = resolved_id {
                            if ui.link(name).clicked() {
                                host.open_block(id, LogicGrid::TYPE_ID);
                            }
                        } else {
                            ui.weak(name);
                        }
                        if resolved_id
                            .and_then(|id| self.solutions.get(&id))
                            .and_then(BlockHandle::read)
                            .is_some_and(|grid| grid.completed())
                        {
                            ui.colored_label(PASSED, ICON_CHECK_CIRCLE.codepoint);
                        }
                        if ui
                            .add_enabled(editable, egui::Button::new(ICON_DELETE).small())
                            .on_hover_text("Remove from this level")
                            .clicked()
                        {
                            remove = Some((*challenge, *solution));
                        }
                    });
                }
                if ui
                    .add_enabled(
                        editable,
                        egui::Button::new(format!("{} New attempt", ICON_ADD.codepoint)),
                    )
                    .test_id(&format!("logic-game.new-attempt.{}", *challenge as usize))
                    .clicked()
                {
                    start = Some((*challenge, solutions.len()));
                }
            });
        }

        if let Some((challenge, solution)) = remove {
            block.operate(LogicGameOperation::RemoveSolution {
                challenge,
                solution,
            });
        }
        if let Some((challenge, index)) = start {
            self.start_solution(challenge, index);
        }
    }
}
