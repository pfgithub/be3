use block_client::{
    blocks::logic_game::{LogicGame, LogicGameOperation, QuizRow},
    BlockHandle,
};
use eframe::egui;

const CELL_WIDTH: f32 = 24.0;
const CELL_HEIGHT: f32 = 24.0;

/// One longhand addition to fill in: the operands as written, and the carry and
/// sum bits the player has to work out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct BinaryAdditionProblem {
    operands: Vec<String>,
    carry_bits: Vec<bool>,
    sum_bits: Vec<bool>,
}

impl BinaryAdditionProblem {
    fn new(operands: &[u64]) -> Self {
        assert!(
            operands.len() >= 2,
            "binary addition problems need at least two operands"
        );
        let total = operands.iter().sum::<u64>();
        let operand_width = operands
            .iter()
            .map(|operand| binary_width(*operand))
            .max()
            .unwrap_or(1);
        let width = operand_width.max(binary_width(total));
        let mut carry_by_column = Vec::with_capacity(width.saturating_sub(1));
        let mut carry = 0u64;
        for column in 0..width {
            if column > 0 {
                carry_by_column.push(carry == 1);
            }
            let column_total = carry
                + operands
                    .iter()
                    .map(|operand| (operand >> column) & 1)
                    .sum::<u64>();
            carry = column_total >> 1;
        }
        carry_by_column.reverse();

        Self {
            operands: operands
                .iter()
                .map(|operand| format!("{operand:0width$b}"))
                .collect(),
            carry_bits: carry_by_column,
            sum_bits: (0..width)
                .rev()
                .map(|column| (total >> column) & 1 == 1)
                .collect(),
        }
    }

    fn width(&self) -> usize {
        self.sum_bits.len()
    }
}

/// The quiz for the level that is worked out on paper. The answers live on the
/// game block, so what is filled in here is what everyone reading the game
/// sees; only which page is open and whether it has been checked are local.
pub(super) struct BinaryAdditionQuiz {
    problems: Vec<BinaryAdditionProblem>,
    current_problem: usize,
    checked: bool,
}

impl Default for BinaryAdditionQuiz {
    fn default() -> Self {
        Self {
            problems: vec![
                BinaryAdditionProblem::new(&[0b10101, 0b01011]),
                BinaryAdditionProblem::new(&[0b11010101, 0b00111110]),
                BinaryAdditionProblem::new(&[0b101101011011, 0b011011010101]),
            ],
            current_problem: 0,
            checked: false,
        }
    }
}

impl BinaryAdditionQuiz {
    /// Room the quiz needs, so the tab can size itself before drawing.
    pub(super) fn height(&self) -> f32 {
        let rows = self.problems[self.current_problem].operands.len() + 5;
        CELL_HEIGHT * rows as f32 + 120.0
    }

    pub(super) fn ui(&mut self, ui: &mut egui::Ui, block: &BlockHandle<LogicGame>) {
        let problem = self.current_problem;
        let (mut carries, mut sums) = self.answers(block, problem);

        ui.label(format!(
            "Problem {} of {}",
            problem + 1,
            self.problems.len()
        ));
        ui.add_space(8.0);
        self.problem_ui(ui, problem, &mut carries, &mut sums, block);
        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui.button("Check").clicked() {
                self.checked = true;
            }
            if ui.button("Reset page").clicked() {
                self.write_row(block, problem, QuizRow::Carries, Vec::new());
                self.write_row(block, problem, QuizRow::Sums, Vec::new());
                self.checked = false;
            }
            if self.checked {
                if self.is_correct(&carries, &sums, problem) {
                    ui.colored_label(
                        egui::Color32::from_rgb(115, 209, 133),
                        "This page is correct.",
                    );
                } else {
                    ui.colored_label(ui.visuals().error_fg_color, "Some blanks still need work.");
                }
            }
        });

        ui.horizontal(|ui| {
            if ui
                .add_enabled(problem > 0, egui::Button::new("Previous"))
                .clicked()
            {
                self.current_problem -= 1;
                self.checked = false;
            }
            let correct = self.is_correct(&carries, &sums, problem);
            if problem + 1 < self.problems.len() {
                if ui.add_enabled(correct, egui::Button::new("Next")).clicked() {
                    self.current_problem += 1;
                    self.checked = false;
                }
            } else if correct && self.all_correct(block) {
                ui.colored_label(
                    egui::Color32::from_rgb(115, 209, 133),
                    "Every problem is correct.",
                );
            }
        });
    }

    /// The stored answers for `problem`, padded to the rows it actually has.
    fn answers(
        &self,
        block: &BlockHandle<LogicGame>,
        problem: usize,
    ) -> (Vec<Option<bool>>, Vec<Option<bool>>) {
        let stored = block.read().and_then(|game| game.quiz(problem).cloned());
        let (carries, sums) = stored
            .map(|answers| (answers.carries, answers.sums))
            .unwrap_or_default();
        (
            fitted(carries, self.problems[problem].carry_bits.len()),
            fitted(sums, self.problems[problem].width()),
        )
    }

    fn is_correct(&self, carries: &[Option<bool>], sums: &[Option<bool>], problem: usize) -> bool {
        let problem = &self.problems[problem];
        matches(carries, &problem.carry_bits) && matches(sums, &problem.sum_bits)
    }

    fn all_correct(&self, block: &BlockHandle<LogicGame>) -> bool {
        (0..self.problems.len()).all(|problem| {
            let (carries, sums) = self.answers(block, problem);
            self.is_correct(&carries, &sums, problem)
        })
    }

    fn write_row(
        &self,
        block: &BlockHandle<LogicGame>,
        problem: usize,
        row: QuizRow,
        values: Vec<Option<bool>>,
    ) {
        block.operate(LogicGameOperation::SetQuizRow {
            problem,
            row,
            values,
        });
    }

    fn problem_ui(
        &self,
        ui: &mut egui::Ui,
        problem_index: usize,
        carries: &mut [Option<bool>],
        sums: &mut [Option<bool>],
        block: &BlockHandle<LogicGame>,
    ) {
        let problem = &self.problems[problem_index];
        ui.group(|ui| {
            ui.add_space(4.0);
            egui::Grid::new(("binary-addition", problem_index))
                .spacing([4.0, 6.0])
                .show(ui, |ui| {
                    ui.label("carry");
                    if bit_buttons(ui, carries, &problem.carry_bits, self.checked) {
                        self.write_row(block, problem_index, QuizRow::Carries, carries.to_vec());
                    }
                    ui.add_sized([CELL_WIDTH, CELL_HEIGHT], egui::Label::new(""));
                    ui.end_row();

                    for (index, operand) in problem.operands.iter().enumerate() {
                        ui.label(if index + 1 == problem.operands.len() {
                            "+"
                        } else {
                            ""
                        });
                        for bit in operand.chars() {
                            ui.monospace(bit.to_string());
                        }
                        ui.end_row();
                    }

                    ui.label("");
                    for _ in 0..problem.width() {
                        ui.separator();
                    }
                    ui.end_row();

                    ui.label("sum");
                    if bit_buttons(ui, sums, &problem.sum_bits, self.checked) {
                        self.write_row(block, problem_index, QuizRow::Sums, sums.to_vec());
                    }
                    ui.end_row();
                });
            ui.add_space(4.0);
        });
    }
}

/// Draws one row of blanks. Returns `true` when a blank was cycled.
fn bit_buttons(
    ui: &mut egui::Ui,
    answers: &mut [Option<bool>],
    expected: &[bool],
    checked: bool,
) -> bool {
    let mut changed = false;
    for (answer, expected) in answers.iter_mut().zip(expected) {
        let correct = *answer == Some(*expected);
        let label = answer.map_or_else(|| " ".to_owned(), |bit| u8::from(bit).to_string());
        let response = ui.add_sized(
            [CELL_WIDTH, CELL_HEIGHT],
            egui::Button::new(egui::RichText::new(label).monospace()),
        );
        if response.clicked() {
            *answer = next_answer(*answer);
            changed = true;
        }
        if checked && !correct {
            ui.painter().rect_stroke(
                response.rect.expand(1.0),
                2.0,
                egui::Stroke::new(1.0_f32, ui.visuals().error_fg_color),
                egui::StrokeKind::Outside,
            );
        }
    }
    changed
}

/// Blank, then 0, then 1, then blank again.
fn next_answer(answer: Option<bool>) -> Option<bool> {
    match answer {
        None => Some(false),
        Some(false) => Some(true),
        Some(true) => None,
    }
}

fn matches(answers: &[Option<bool>], expected: &[bool]) -> bool {
    answers.len() == expected.len()
        && answers
            .iter()
            .zip(expected)
            .all(|(answer, expected)| *answer == Some(*expected))
}

/// Stored rows are whatever length they were written at, so they are trimmed or
/// padded to the row the problem actually has.
fn fitted(mut values: Vec<Option<bool>>, length: usize) -> Vec<Option<bool>> {
    values.resize(length, None);
    values
}

fn binary_width(value: u64) -> usize {
    let width = u64::BITS - value.leading_zeros();
    width.max(1) as usize
}

#[cfg(test)]
mod tests;
