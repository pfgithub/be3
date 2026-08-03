use eframe::egui;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryAdditionProblem {
    operands: Vec<String>,
    carry_bits: Vec<char>,
    sum_bits: Vec<char>,
}

impl BinaryAdditionProblem {
    pub fn new(operands: &[u64]) -> Self {
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
                carry_by_column.push(char::from_digit(carry as u32, 2).expect("carry is binary"));
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
            sum_bits: format!("{total:0width$b}").chars().collect(),
        }
    }

    fn width(&self) -> usize {
        self.sum_bits.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BinaryAdditionQuiz {
    problems: Vec<BinaryAdditionProblem>,
    carry_answers: Vec<Vec<Option<char>>>,
    sum_answers: Vec<Vec<Option<char>>>,
    current_problem: usize,
    checked: bool,
    passed_event: bool,
}

impl Default for BinaryAdditionQuiz {
    fn default() -> Self {
        let problems = vec![
            BinaryAdditionProblem::new(&[0b10101, 0b01011]),
            BinaryAdditionProblem::new(&[0b11010101, 0b00111110]),
            BinaryAdditionProblem::new(&[0b101101011011, 0b011011010101]),
        ];
        let carry_answers = blank_carry_answers(&problems);
        let sum_answers = blank_sum_answers(&problems);
        Self {
            problems,
            carry_answers,
            sum_answers,
            current_problem: 0,
            checked: false,
            passed_event: false,
        }
    }
}

impl BinaryAdditionQuiz {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        egui::Frame::central_panel(ui.style()).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical_centered_justified(|ui| {
                    ui.heading("Binary Addition");
                });
                ui.add_space(12.0);
                ui.label("Fill in each carry bit and each output bit.");
                ui.add_space(12.0);

                ui.label(format!(
                    "Problem {} of {}",
                    self.current_problem + 1,
                    self.problems.len()
                ));
                ui.add_space(8.0);
                self.problem_ui(ui, self.current_problem);
                ui.add_space(18.0);

                ui.horizontal(|ui| {
                    if ui.button("Check").clicked() {
                        self.checked = true;
                    }
                    if ui.button("Reset Page").clicked() {
                        self.reset_current_page();
                    }
                    if ui.button("Reset All").clicked() {
                        self.reset();
                    }
                    let current_correct = self.is_current_problem_correct();
                    if self.checked {
                        if current_correct {
                            ui.colored_label(
                                egui::Color32::from_rgb(115, 209, 133),
                                "This page is correct.",
                            );
                        } else {
                            ui.colored_label(
                                ui.visuals().error_fg_color,
                                "Some blanks still need work.",
                            );
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(self.current_problem > 0, egui::Button::new("Previous"))
                        .clicked()
                    {
                        self.current_problem -= 1;
                        self.checked = false;
                    }

                    let current_correct = self.is_current_problem_correct();
                    let on_last_problem = self.current_problem + 1 == self.problems.len();
                    if on_last_problem {
                        if ui
                            .add_enabled(
                                current_correct && self.is_complete_and_correct(),
                                egui::Button::new("Finish"),
                            )
                            .clicked()
                        {
                            self.checked = true;
                            self.passed_event = true;
                        }
                    } else if ui
                        .add_enabled(current_correct, egui::Button::new("Next"))
                        .clicked()
                    {
                        self.current_problem += 1;
                        self.checked = false;
                    }
                });
            });
        });
    }

    pub fn take_passed(&mut self) -> bool {
        let passed = self.passed_event;
        self.passed_event = false;
        passed
    }

    pub fn is_complete_and_correct(&self) -> bool {
        self.problems
            .iter()
            .enumerate()
            .all(|(problem_index, problem)| {
                bit_row_matches(&self.carry_answers[problem_index], &problem.carry_bits)
                    && bit_row_matches(&self.sum_answers[problem_index], &problem.sum_bits)
            })
    }

    fn is_current_problem_correct(&self) -> bool {
        let problem = &self.problems[self.current_problem];
        bit_row_matches(
            &self.carry_answers[self.current_problem],
            &problem.carry_bits,
        ) && bit_row_matches(&self.sum_answers[self.current_problem], &problem.sum_bits)
    }

    fn problem_ui(&mut self, ui: &mut egui::Ui, problem_index: usize) {
        let problem = &self.problems[problem_index];
        let cell_width = 24.0;
        ui.group(|ui| {
            ui.add_space(4.0);
            egui::Grid::new(("binary-addition", problem_index))
                .spacing([4.0, 6.0])
                .show(ui, |ui| {
                    ui.label("carry");
                    bit_buttons(
                        ui,
                        &mut self.carry_answers[problem_index],
                        &problem.carry_bits,
                        self.checked,
                        cell_width,
                    );
                    ui.add_sized([cell_width, 24.0], egui::Label::new(""));
                    ui.end_row();

                    for operand_index in 0..problem.operands.len() {
                        ui.label(if operand_index + 1 == problem.operands.len() {
                            "+"
                        } else {
                            ""
                        });
                        for bit in problem.operands[operand_index].chars() {
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
                    bit_buttons(
                        ui,
                        &mut self.sum_answers[problem_index],
                        &problem.sum_bits,
                        self.checked,
                        cell_width,
                    );
                    ui.end_row();
                });
            ui.add_space(4.0);
        });
    }

    fn reset(&mut self) {
        self.carry_answers = blank_carry_answers(&self.problems);
        self.sum_answers = blank_sum_answers(&self.problems);
        self.current_problem = 0;
        self.checked = false;
        self.passed_event = false;
    }

    fn reset_current_page(&mut self) {
        self.carry_answers[self.current_problem] =
            vec![None; self.problems[self.current_problem].carry_bits.len()];
        self.sum_answers[self.current_problem] =
            vec![None; self.problems[self.current_problem].sum_bits.len()];
        self.checked = false;
        self.passed_event = false;
    }
}

fn bit_buttons(
    ui: &mut egui::Ui,
    answers: &mut [Option<char>],
    expected: &[char],
    checked: bool,
    cell_width: f32,
) {
    for (answer, expected) in answers.iter_mut().zip(expected) {
        let correct = *answer == Some(*expected);
        let label = answer.map_or(" ".to_owned(), |bit| bit.to_string());
        let response = ui.add_sized(
            [cell_width, 24.0],
            egui::Button::new(egui::RichText::new(label).monospace()),
        );
        if response.clicked() {
            *answer = next_bit_answer(*answer);
        }
        if checked && !correct {
            let rect = response.rect.expand(1.0);
            ui.painter().rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(1.0_f32, ui.visuals().error_fg_color),
                egui::StrokeKind::Outside,
            );
        }
    }
}

fn next_bit_answer(answer: Option<char>) -> Option<char> {
    match answer {
        None => Some('0'),
        Some('0') => Some('1'),
        Some('1') => None,
        _ => None,
    }
}

fn bit_row_matches(answers: &[Option<char>], expected: &[char]) -> bool {
    answers.len() == expected.len()
        && answers
            .iter()
            .zip(expected)
            .all(|(answer, expected)| *answer == Some(*expected))
}

fn blank_carry_answers(problems: &[BinaryAdditionProblem]) -> Vec<Vec<Option<char>>> {
    problems
        .iter()
        .map(|problem| vec![None; problem.carry_bits.len()])
        .collect()
}

fn blank_sum_answers(problems: &[BinaryAdditionProblem]) -> Vec<Vec<Option<char>>> {
    problems
        .iter()
        .map(|problem| vec![None; problem.width()])
        .collect()
}

fn binary_width(value: u64) -> usize {
    let width = u64::BITS - value.leading_zeros();
    width.max(1) as usize
}

#[cfg(test)]
mod tests;
