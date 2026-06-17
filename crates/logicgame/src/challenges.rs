use std::{collections::BTreeMap, fmt};

use crate::{
    execution::Vm,
    grid::{value_mask, ComponentKind, InputId, LogicGrid, OutputId, Scale, ValidationError},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChallengeId {
    Nor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengePortKind {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengePort {
    pub label: &'static str,
    pub kind: ChallengePortKind,
    pub scale: Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeValue {
    pub label: &'static str,
    pub value: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengeTestCase {
    pub name: &'static str,
    pub inputs: &'static [ChallengeValue],
    pub outputs: &'static [ChallengeValue],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Challenge {
    pub id: ChallengeId,
    pub name: &'static str,
    pub goal: &'static str,
    pub ports: &'static [ChallengePort],
    pub tests: &'static [ChallengeTestCase],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeTestResult {
    pub name: String,
    pub inputs: Vec<(String, u64)>,
    pub expected_outputs: Vec<(String, u64)>,
    pub actual_outputs: Vec<(String, Option<u64>)>,
    pub passed: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChallengeGridError {
    InvalidGrid(ValidationError),
    UnavailableComponent(&'static str),
    InvalidScale { label: &'static str, scale: Scale },
    UnknownInput(InputId),
    UnknownOutput(OutputId),
    DuplicateInput(&'static str),
    DuplicateOutput(&'static str),
    MissingInput(&'static str),
    MissingOutput(&'static str),
}

impl fmt::Display for ChallengeGridError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidGrid(error) => write!(formatter, "{error:?}"),
            Self::UnavailableComponent(name) => write!(formatter, "{name} is not available"),
            Self::InvalidScale { label, scale } => {
                write!(formatter, "{label} must be {}x", Scale::ONE.get())?;
                write!(formatter, " but is {}x", scale.get())
            }
            Self::UnknownInput(id) => {
                write!(formatter, "Input {} is not part of the challenge", id.0)
            }
            Self::UnknownOutput(id) => {
                write!(formatter, "Output {} is not part of the challenge", id.0)
            }
            Self::DuplicateInput(label) => {
                write!(formatter, "Input {label} appears more than once")
            }
            Self::DuplicateOutput(label) => {
                write!(formatter, "Output {label} appears more than once")
            }
            Self::MissingInput(label) => write!(formatter, "Input {label} is missing"),
            Self::MissingOutput(label) => write!(formatter, "Output {label} is missing"),
        }
    }
}

const NOR_PORTS: [ChallengePort; 3] = [
    ChallengePort {
        label: "A",
        kind: ChallengePortKind::Input,
        scale: Scale::ONE,
    },
    ChallengePort {
        label: "B",
        kind: ChallengePortKind::Input,
        scale: Scale::ONE,
    },
    ChallengePort {
        label: "OUT",
        kind: ChallengePortKind::Output,
        scale: Scale::ONE,
    },
];

const NOR_TEST_00_INPUTS: [ChallengeValue; 2] = [
    ChallengeValue {
        label: "A",
        value: 0,
    },
    ChallengeValue {
        label: "B",
        value: 0,
    },
];
const NOR_TEST_00_OUTPUTS: [ChallengeValue; 1] = [ChallengeValue {
    label: "OUT",
    value: 1,
}];
const NOR_TEST_01_INPUTS: [ChallengeValue; 2] = [
    ChallengeValue {
        label: "A",
        value: 0,
    },
    ChallengeValue {
        label: "B",
        value: 1,
    },
];
const NOR_TEST_01_OUTPUTS: [ChallengeValue; 1] = [ChallengeValue {
    label: "OUT",
    value: 0,
}];
const NOR_TEST_10_INPUTS: [ChallengeValue; 2] = [
    ChallengeValue {
        label: "A",
        value: 1,
    },
    ChallengeValue {
        label: "B",
        value: 0,
    },
];
const NOR_TEST_10_OUTPUTS: [ChallengeValue; 1] = [ChallengeValue {
    label: "OUT",
    value: 0,
}];
const NOR_TEST_11_INPUTS: [ChallengeValue; 2] = [
    ChallengeValue {
        label: "A",
        value: 1,
    },
    ChallengeValue {
        label: "B",
        value: 1,
    },
];
const NOR_TEST_11_OUTPUTS: [ChallengeValue; 1] = [ChallengeValue {
    label: "OUT",
    value: 0,
}];

const NOR_TESTS: [ChallengeTestCase; 4] = [
    ChallengeTestCase {
        name: "00 -> 1",
        inputs: &NOR_TEST_00_INPUTS,
        outputs: &NOR_TEST_00_OUTPUTS,
    },
    ChallengeTestCase {
        name: "01 -> 0",
        inputs: &NOR_TEST_01_INPUTS,
        outputs: &NOR_TEST_01_OUTPUTS,
    },
    ChallengeTestCase {
        name: "10 -> 0",
        inputs: &NOR_TEST_10_INPUTS,
        outputs: &NOR_TEST_10_OUTPUTS,
    },
    ChallengeTestCase {
        name: "11 -> 0",
        inputs: &NOR_TEST_11_INPUTS,
        outputs: &NOR_TEST_11_OUTPUTS,
    },
];

pub const NOR_CHALLENGE: Challenge = Challenge {
    id: ChallengeId::Nor,
    name: "NOR",
    goal: "Build a circuit where OUT is on only when A and B are both off.",
    ports: &NOR_PORTS,
    tests: &NOR_TESTS,
};

pub const CHALLENGES: [Challenge; 1] = [NOR_CHALLENGE];

pub fn challenge(id: ChallengeId) -> &'static Challenge {
    match id {
        ChallengeId::Nor => &NOR_CHALLENGE,
    }
}

pub fn input_id(challenge: &Challenge, label: &str) -> Option<InputId> {
    challenge
        .ports
        .iter()
        .filter(|port| port.kind == ChallengePortKind::Input)
        .position(|port| port.label == label)
        .map(InputId)
}

pub fn output_id(challenge: &Challenge, label: &str) -> Option<OutputId> {
    challenge
        .ports
        .iter()
        .filter(|port| port.kind == ChallengePortKind::Output)
        .position(|port| port.label == label)
        .map(OutputId)
}

pub fn input_label(challenge: &Challenge, id: InputId) -> Option<&'static str> {
    challenge
        .ports
        .iter()
        .filter(|port| port.kind == ChallengePortKind::Input)
        .nth(id.0)
        .map(|port| port.label)
}

pub fn output_label(challenge: &Challenge, id: OutputId) -> Option<&'static str> {
    challenge
        .ports
        .iter()
        .filter(|port| port.kind == ChallengePortKind::Output)
        .nth(id.0)
        .map(|port| port.label)
}

pub fn validate_challenge_grid(challenge: &Challenge, grid: &LogicGrid) -> Vec<ChallengeGridError> {
    let mut errors = grid
        .validate()
        .into_iter()
        .map(ChallengeGridError::InvalidGrid)
        .collect::<Vec<_>>();
    let mut inputs = BTreeMap::<&'static str, usize>::new();
    let mut outputs = BTreeMap::<&'static str, usize>::new();

    for component in grid.components() {
        match component.kind {
            ComponentKind::Not { scale } => {
                if scale != Scale::ONE {
                    errors.push(ChallengeGridError::InvalidScale {
                        label: "NOT gate",
                        scale,
                    });
                }
            }
            ComponentKind::Input { scale, id } => match input_label(challenge, id) {
                Some(label) => {
                    if scale != Scale::ONE {
                        errors.push(ChallengeGridError::InvalidScale { label, scale });
                    }
                    let count = inputs.entry(label).or_default();
                    *count += 1;
                    if *count > 1 {
                        errors.push(ChallengeGridError::DuplicateInput(label));
                    }
                }
                None => errors.push(ChallengeGridError::UnknownInput(id)),
            },
            ComponentKind::Output { scale, id } => match output_label(challenge, id) {
                Some(label) => {
                    if scale != Scale::ONE {
                        errors.push(ChallengeGridError::InvalidScale { label, scale });
                    }
                    let count = outputs.entry(label).or_default();
                    *count += 1;
                    if *count > 1 {
                        errors.push(ChallengeGridError::DuplicateOutput(label));
                    }
                }
                None => errors.push(ChallengeGridError::UnknownOutput(id)),
            },
            ComponentKind::MergerSplitter { .. } => {
                errors.push(ChallengeGridError::UnavailableComponent("Merger/Splitter"));
            }
            ComponentKind::Led => errors.push(ChallengeGridError::UnavailableComponent("LED")),
            ComponentKind::Storage { .. } => {
                errors.push(ChallengeGridError::UnavailableComponent("Storage"));
            }
            ComponentKind::Subcomponent { .. } => {
                errors.push(ChallengeGridError::UnavailableComponent("Subcomponent"));
            }
        }
    }

    for port in challenge.ports {
        match port.kind {
            ChallengePortKind::Input => {
                if !inputs.contains_key(port.label) {
                    errors.push(ChallengeGridError::MissingInput(port.label));
                }
            }
            ChallengePortKind::Output => {
                if !outputs.contains_key(port.label) {
                    errors.push(ChallengeGridError::MissingOutput(port.label));
                }
            }
        }
    }

    for wire in grid.wires() {
        if wire.scale != Scale::ONE {
            errors.push(ChallengeGridError::InvalidScale {
                label: "Wire",
                scale: wire.scale,
            });
        }
    }

    errors
}

pub fn run_challenge_tests(challenge: &Challenge, grid: &LogicGrid) -> Vec<ChallengeTestResult> {
    let validation_errors = validate_challenge_grid(challenge, grid);
    if !validation_errors.is_empty() {
        let error = validation_errors
            .into_iter()
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        return challenge
            .tests
            .iter()
            .map(|test| failed_result(test, Some(error.clone())))
            .collect();
    }

    let graph = grid.generate_graph();
    let vm = match Vm::from_graph(grid, &graph) {
        Ok(vm) => vm,
        Err(error) => {
            let error = format!("{error:?}");
            return challenge
                .tests
                .iter()
                .map(|test| failed_result(test, Some(error.clone())))
                .collect();
        }
    };

    challenge
        .tests
        .iter()
        .map(|test| run_test_case(challenge, vm.clone(), test))
        .collect()
}

fn run_test_case(
    challenge: &Challenge,
    mut vm: Vm,
    test: &ChallengeTestCase,
) -> ChallengeTestResult {
    vm.begin_tick();
    for input in test.inputs {
        let Some(id) = input_id(challenge, input.label) else {
            return failed_result(test, Some(format!("Unknown input {}", input.label)));
        };
        let Some(&address) = vm.input_addresses().get(id.0) else {
            return failed_result(test, Some(format!("Missing input {}", input.label)));
        };
        if address >= vm.root_component.memory_size {
            return failed_result(
                test,
                Some(format!("Input {} is not connected", input.label)),
            );
        }
        vm.root_memory_mut()[address] |= input.value & value_mask(Scale::ONE);
    }
    vm.execute();

    let mut actual_outputs = Vec::new();
    let mut passed = true;
    for output in test.outputs {
        let Some(id) = output_id(challenge, output.label) else {
            return failed_result(test, Some(format!("Unknown output {}", output.label)));
        };
        let actual = vm
            .output_addresses()
            .get(id.0)
            .and_then(|&address| vm.root_memory().get(address).copied())
            .map(|value| value & value_mask(Scale::ONE));
        passed &= actual == Some(output.value);
        actual_outputs.push((output.label.to_owned(), actual));
    }

    ChallengeTestResult {
        name: test.name.to_owned(),
        inputs: test_inputs(test),
        expected_outputs: test_outputs(test),
        actual_outputs,
        passed,
        error: None,
    }
}

fn failed_result(test: &ChallengeTestCase, error: Option<String>) -> ChallengeTestResult {
    ChallengeTestResult {
        name: test.name.to_owned(),
        inputs: test_inputs(test),
        expected_outputs: test_outputs(test),
        actual_outputs: test
            .outputs
            .iter()
            .map(|output| (output.label.to_owned(), None))
            .collect(),
        passed: false,
        error,
    }
}

fn test_inputs(test: &ChallengeTestCase) -> Vec<(String, u64)> {
    test.inputs
        .iter()
        .map(|input| (input.label.to_owned(), input.value))
        .collect()
}

fn test_outputs(test: &ChallengeTestCase) -> Vec<(String, u64)> {
    test.outputs
        .iter()
        .map(|output| (output.label.to_owned(), output.value))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::grid::{ComponentKind, Point, Rotation, Wire};

    use super::*;

    fn wire(start: (i64, i64), end: (i64, i64)) -> Wire {
        Wire::new(
            Point::new(start.0, start.1),
            Point::new(end.0, end.1),
            Scale::ONE,
        )
        .unwrap()
    }

    #[test]
    fn nor_challenge_defines_ports_and_truth_table() {
        assert_eq!(NOR_CHALLENGE.ports.len(), 3);
        assert_eq!(input_id(&NOR_CHALLENGE, "A"), Some(InputId(0)));
        assert_eq!(input_id(&NOR_CHALLENGE, "B"), Some(InputId(1)));
        assert_eq!(output_id(&NOR_CHALLENGE, "OUT"), Some(OutputId(0)));
        assert!(NOR_CHALLENGE
            .ports
            .iter()
            .all(|port| port.scale == Scale::ONE));
        assert_eq!(NOR_CHALLENGE.tests.len(), 4);
        assert_eq!(
            NOR_CHALLENGE
                .tests
                .iter()
                .map(|test| test.outputs[0].value)
                .collect::<Vec<_>>(),
            vec![1, 0, 0, 0]
        );
    }

    #[test]
    fn challenge_validation_rejects_unavailable_components_scales_and_duplicates() {
        let mut grid = LogicGrid::new();
        grid.add_component(
            Point::new(0, 0),
            Rotation::Up,
            ComponentKind::Storage {
                scale: Scale::ONE,
                value: 0,
            },
        );
        grid.add_component_with_explicit_io(
            Point::new(2, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::new(2).unwrap(),
                id: InputId(0),
            },
        );
        grid.add_component_with_explicit_io(
            Point::new(6, 0),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(0),
            },
        );

        let errors = validate_challenge_grid(&NOR_CHALLENGE, &grid);
        assert!(errors.contains(&ChallengeGridError::UnavailableComponent("Storage")));
        assert!(errors.contains(&ChallengeGridError::InvalidScale {
            label: "A",
            scale: Scale::new(2).unwrap(),
        }));
        assert!(errors.contains(&ChallengeGridError::DuplicateInput("A")));
    }

    #[test]
    fn challenge_test_runner_passes_valid_nor_and_fails_wrong_circuit() {
        let mut valid = LogicGrid::new();
        valid.add_component_with_explicit_io(
            Point::new(0, 4),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(0),
            },
        );
        valid.add_component_with_explicit_io(
            Point::new(2, 4),
            Rotation::Up,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(1),
            },
        );
        valid.add_component(
            Point::new(4, 1),
            Rotation::Up,
            ComponentKind::Not { scale: Scale::ONE },
        );
        valid.add_component_with_explicit_io(
            Point::new(4, -2),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId(0),
            },
        );
        valid.add_wire(wire((0, 5), (4, 5)));
        valid.add_wire(wire((4, 3), (4, 5)));
        valid.add_wire(wire((4, -1), (4, 1)));

        let results = run_challenge_tests(&NOR_CHALLENGE, &valid);
        assert!(results.iter().all(|result| result.passed), "{results:?}");

        let mut wrong = LogicGrid::new();
        wrong.add_component_with_explicit_io(
            Point::new(0, 2),
            Rotation::Right,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(0),
            },
        );
        wrong.add_component_with_explicit_io(
            Point::new(0, 4),
            Rotation::Right,
            ComponentKind::Input {
                scale: Scale::ONE,
                id: InputId(1),
            },
        );
        wrong.add_component_with_explicit_io(
            Point::new(4, 2),
            Rotation::Up,
            ComponentKind::Output {
                scale: Scale::ONE,
                id: OutputId(0),
            },
        );
        wrong.add_wire(wire((1, 2), (4, 2)));
        let results = run_challenge_tests(&NOR_CHALLENGE, &wrong);
        assert!(results.iter().any(|result| !result.passed));
    }
}
