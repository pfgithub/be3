use crate::grid::{InputId, OutputId, Scale};
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChallengeId {
    Nor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChallengeComponentKind {
    MergerSplitter,
    Led,
    Storage,
    Subcomponent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChallengePort {
    pub input_id: Option<InputId>,
    pub output_id: Option<OutputId>,
    pub label: &'static str,
    pub scale: Scale,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeValue {
    pub label: &'static str,
    pub value: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChallengeTick {
    pub inputs: Vec<ChallengeValue>,
    pub outputs: Vec<ChallengeValue>,
}

#[derive(Clone, Copy)]
pub struct Challenge {
    pub id: ChallengeId,
    pub name: &'static str,
    pub goal: &'static str,
    pub inputs: &'static [ChallengePort],
    pub outputs: &'static [ChallengePort],
    pub disallowed_components: &'static [ChallengeComponentKind],
    pub validation_ticks: usize,
    pub generate: fn(&mut dyn RngCore) -> ChallengeTick,
}

impl std::fmt::Debug for Challenge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Challenge")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("goal", &self.goal)
            .field("inputs", &self.inputs)
            .field("outputs", &self.outputs)
            .field("disallowed_components", &self.disallowed_components)
            .field("validation_ticks", &self.validation_ticks)
            .finish_non_exhaustive()
    }
}

impl PartialEq for Challenge {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.name == other.name
            && self.goal == other.goal
            && self.inputs == other.inputs
            && self.outputs == other.outputs
            && self.disallowed_components == other.disallowed_components
            && self.validation_ticks == other.validation_ticks
    }
}

impl Eq for Challenge {}

const NOR_INPUTS: [ChallengePort; 2] = [
    ChallengePort {
        input_id: Some(InputId::from_u128(1)),
        output_id: None,
        label: "A",
        scale: Scale::ONE,
    },
    ChallengePort {
        input_id: Some(InputId::from_u128(2)),
        output_id: None,
        label: "B",
        scale: Scale::ONE,
    },
];

const NOR_OUTPUTS: [ChallengePort; 1] = [ChallengePort {
    input_id: None,
    output_id: Some(OutputId::from_u128(1)),
    label: "OUT",
    scale: Scale::ONE,
}];

const NOR_DISALLOWED_COMPONENTS: [ChallengeComponentKind; 4] = [
    ChallengeComponentKind::MergerSplitter,
    ChallengeComponentKind::Led,
    ChallengeComponentKind::Storage,
    ChallengeComponentKind::Subcomponent,
];

pub const NOR_CHALLENGE: Challenge = Challenge {
    id: ChallengeId::Nor,
    name: "NOR",
    goal: "Build a circuit where OUT is on only when A and B are both off.",
    inputs: &NOR_INPUTS,
    outputs: &NOR_OUTPUTS,
    disallowed_components: &NOR_DISALLOWED_COMPONENTS,
    validation_ticks: 1000,
    generate: generate_nor_tick,
};

pub const CHALLENGES: [Challenge; 1] = [NOR_CHALLENGE];

pub fn challenge(id: ChallengeId) -> &'static Challenge {
    match id {
        ChallengeId::Nor => &NOR_CHALLENGE,
    }
}

pub fn input_id(challenge: &Challenge, label: &str) -> Option<InputId> {
    challenge
        .inputs
        .iter()
        .find(|port| port.label == label)
        .and_then(|port| port.input_id)
}

pub fn output_id(challenge: &Challenge, label: &str) -> Option<OutputId> {
    challenge
        .outputs
        .iter()
        .find(|port| port.label == label)
        .and_then(|port| port.output_id)
}

pub fn input_index(challenge: &Challenge, id: InputId) -> Option<usize> {
    challenge
        .inputs
        .iter()
        .position(|port| port.input_id == Some(id))
}

pub fn output_index(challenge: &Challenge, id: OutputId) -> Option<usize> {
    challenge
        .outputs
        .iter()
        .position(|port| port.output_id == Some(id))
}

pub fn input_label(challenge: &Challenge, id: InputId) -> Option<&'static str> {
    challenge
        .inputs
        .iter()
        .find(|port| port.input_id == Some(id))
        .map(|port| port.label)
}

pub fn output_label(challenge: &Challenge, id: OutputId) -> Option<&'static str> {
    challenge
        .outputs
        .iter()
        .find(|port| port.output_id == Some(id))
        .map(|port| port.label)
}

fn generate_nor_tick(rng: &mut dyn RngCore) -> ChallengeTick {
    let a = rng.next_u64() & 1;
    let b = rng.next_u64() & 1;
    let out = u64::from((a | b) == 0);
    ChallengeTick {
        inputs: vec![
            ChallengeValue {
                label: "A",
                value: a,
            },
            ChallengeValue {
                label: "B",
                value: b,
            },
        ],
        outputs: vec![ChallengeValue {
            label: "OUT",
            value: out,
        }],
    }
}
