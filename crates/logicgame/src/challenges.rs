use crate::grid::Scale;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChallengeId {
    Nor,
    Or,
    Nand,
    And,
    Xor,
    Transistor,
    Adder,
    Adder2,
    Adder4,
    SrLatch,
    TFlipFlop,
}

impl ChallengeId {
    pub fn name(self) -> &'static str {
        match self {
            ChallengeId::Nor => "NOR",
            ChallengeId::Or => "OR",
            ChallengeId::Nand => "NAND",
            ChallengeId::And => "AND",
            ChallengeId::Xor => "XOR",
            ChallengeId::Transistor => "Transistor",
            ChallengeId::Adder => "Adder",
            ChallengeId::Adder2 => "2-bit Adder",
            ChallengeId::Adder4 => "4-bit Adder",
            ChallengeId::SrLatch => "SR Latch",
            ChallengeId::TFlipFlop => "T Flip-Flop",
        }
    }
}

#[derive(Debug)]
pub struct ChallengePort {
    pub label: &'static str,
    pub scale: Scale,
    pub values: Vec<u64>,
}

#[derive(Debug)]
pub struct Challenge {
    pub goal: String,
    pub inputs: Vec<ChallengePort>,
    pub outputs: Vec<ChallengePort>,
    pub ticks: usize,
}

/// Builds a combinational two-input, one-output challenge over random single-bit
/// inputs, where `op` maps the inputs to the expected output for each tick.
fn binary_gate_challenge(
    rng: &mut impl RngCore,
    goal: &str,
    op: impl Fn(u64, u64) -> u64,
) -> Challenge {
    let mut input_a: Vec<u64> = vec![];
    let mut input_b: Vec<u64> = vec![];
    let mut output: Vec<u64> = vec![];
    for _ in 0..1000 {
        let a = rng.next_u64() & 1;
        let b = rng.next_u64() & 1;
        input_a.push(a);
        input_b.push(b);
        output.push(op(a, b));
    }
    Challenge {
        goal: goal.to_string(),
        inputs: vec![
            ChallengePort {
                label: "A",
                scale: Scale::ONE,
                values: input_a,
            },
            ChallengePort {
                label: "B",
                scale: Scale::ONE,
                values: input_b,
            },
        ],
        outputs: vec![ChallengePort {
            label: "OUT",
            scale: Scale::ONE,
            values: output,
        }],
        ticks: 1000,
    }
}

/// Builds a ripple-carry adder challenge for `width`-bit operands plus a carry-in,
/// exhaustively covering every combination of inputs.
fn adder_challenge(width: u8, goal: &str) -> Challenge {
    let scale = Scale::new(width).expect("adder width is a valid scale");
    let span = 1u64 << width;
    let mask = span - 1;

    let mut a_values: Vec<u64> = vec![];
    let mut b_values: Vec<u64> = vec![];
    let mut carry_in: Vec<u64> = vec![];
    let mut sum_values: Vec<u64> = vec![];
    let mut carry_out: Vec<u64> = vec![];
    for a in 0..span {
        for b in 0..span {
            for cin in 0..2 {
                let total = a + b + cin;
                a_values.push(a);
                b_values.push(b);
                carry_in.push(cin);
                sum_values.push(total & mask);
                carry_out.push(total >> width);
            }
        }
    }
    let ticks = a_values.len();
    Challenge {
        goal: goal.to_string(),
        inputs: vec![
            ChallengePort {
                label: "A",
                scale,
                values: a_values,
            },
            ChallengePort {
                label: "B",
                scale,
                values: b_values,
            },
            ChallengePort {
                label: "CARRY IN",
                scale: Scale::ONE,
                values: carry_in,
            },
        ],
        outputs: vec![
            ChallengePort {
                label: "SUM",
                scale,
                values: sum_values,
            },
            ChallengePort {
                label: "CARRY OUT",
                scale: Scale::ONE,
                values: carry_out,
            },
        ],
        ticks,
    }
}

pub fn generate_challenge(challenge: ChallengeId) -> Challenge {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);

    match challenge {
        ChallengeId::Nor => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on only when A and B are both off.",
            |a, b| u64::from((a | b) == 0),
        ),
        ChallengeId::Or => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on when A or B (or both) are on.",
            |a, b| a | b,
        ),
        ChallengeId::Nand => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on unless A and B are both on.",
            |a, b| u64::from((a & b) == 0),
        ),
        ChallengeId::And => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on only when A and B are both on.",
            |a, b| a & b,
        ),
        ChallengeId::Xor => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on when exactly one of A and B is on.",
            |a, b| a ^ b,
        ),
        ChallengeId::Transistor => binary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT passes A through when the gate B is on, otherwise OUT is off.",
            |a, b| if b == 1 { a } else { 0 },
        ),
        ChallengeId::Adder => adder_challenge(
            1,
            "Build a full adder: SUM and CARRY OUT must equal A + B + CARRY IN.",
        ),
        ChallengeId::Adder2 => adder_challenge(
            2,
            "Build a 2-bit adder: SUM and CARRY OUT must equal A + B + CARRY IN.",
        ),
        ChallengeId::Adder4 => adder_challenge(
            4,
            "Build a 4-bit adder: SUM and CARRY OUT must equal A + B + CARRY IN.",
        ),
        ChallengeId::SrLatch => {
            let mut set: Vec<u64> = vec![];
            let mut reset: Vec<u64> = vec![];
            let mut value: Vec<u64> = vec![];
            let mut state = 0u64;
            for _ in 0..1000 {
                let s = rng.next_u64() & 1;
                let r = rng.next_u64() & 1;
                set.push(s);
                reset.push(r);
                // VALUE reflects the state stored from the previous tick; this tick's
                // SET/RESET only take effect on the following tick.
                value.push(state);
                state = if s == 1 {
                    1
                } else if r == 1 {
                    0
                } else {
                    state
                };
            }
            Challenge {
                goal: "Build an SR latch: SET turns VALUE on, RESET turns it off, and \
                    VALUE holds otherwise. SET wins if both are on. VALUE updates one tick \
                    after its inputs."
                    .to_string(),
                inputs: vec![
                    ChallengePort {
                        label: "SET",
                        scale: Scale::ONE,
                        values: set,
                    },
                    ChallengePort {
                        label: "RESET",
                        scale: Scale::ONE,
                        values: reset,
                    },
                ],
                outputs: vec![ChallengePort {
                    label: "VALUE",
                    scale: Scale::ONE,
                    values: value,
                }],
                ticks: 1000,
            }
        }
        ChallengeId::TFlipFlop => {
            let mut toggle: Vec<u64> = vec![];
            let mut value: Vec<u64> = vec![];
            let mut state = 0u64;
            for _ in 0..1000 {
                let t = rng.next_u64() & 1;
                toggle.push(t);
                // VALUE reflects the state stored from the previous tick; this tick's
                // TOGGLE only flips the state on the following tick.
                value.push(state);
                state ^= t;
            }
            Challenge {
                goal: "Build a T flip-flop: each tick TOGGLE is on, VALUE flips. \
                    VALUE updates one tick after its input."
                    .to_string(),
                inputs: vec![ChallengePort {
                    label: "TOGGLE",
                    scale: Scale::ONE,
                    values: toggle,
                }],
                outputs: vec![ChallengePort {
                    label: "VALUE",
                    scale: Scale::ONE,
                    values: value,
                }],
                ticks: 1000,
            }
        }
    }
}

pub const CHALLENGES: [ChallengeId; 11] = [
    ChallengeId::Nor,
    ChallengeId::Or,
    ChallengeId::Nand,
    ChallengeId::And,
    ChallengeId::Xor,
    ChallengeId::Transistor,
    ChallengeId::Adder,
    ChallengeId::Adder2,
    ChallengeId::Adder4,
    ChallengeId::SrLatch,
    ChallengeId::TFlipFlop,
];
