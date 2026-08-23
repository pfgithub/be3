use crate::grid::Scale;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChallengeId {
    Nor,
    Or,
    Nand,
    And,
    And3,
    Xor,
    Transistor,
    TwoTickDelay,
    FlipOneBit,
    Adder,
    Adder2,
    Adder4,
    SrLatch,
    TFlipFlop,
    MemoryCell,
    BinaryAddition,
}

impl ChallengeId {
    pub fn name(self) -> &'static str {
        match self {
            ChallengeId::Nor => "NOR",
            ChallengeId::Or => "OR",
            ChallengeId::Nand => "NAND",
            ChallengeId::And => "AND",
            ChallengeId::And3 => "3-AND",
            ChallengeId::Xor => "XOR",
            ChallengeId::Transistor => "Transistor",
            ChallengeId::TwoTickDelay => "Two Tick Delay",
            ChallengeId::FlipOneBit => "Flip One Bit",
            ChallengeId::Adder => "Adder",
            ChallengeId::Adder2 => "2-bit Adder",
            ChallengeId::Adder4 => "4-bit Adder",
            ChallengeId::SrLatch => "SR Latch",
            ChallengeId::TFlipFlop => "T Flip-Flop",
            ChallengeId::MemoryCell => "Memory Cell",
            ChallengeId::BinaryAddition => "Binary Addition",
        }
    }

    pub fn is_special(self) -> bool {
        matches!(self, Self::BinaryAddition)
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

fn ternary_gate_challenge(
    rng: &mut impl RngCore,
    goal: &str,
    op: impl Fn(u64, u64, u64) -> u64,
) -> Challenge {
    let mut input_a: Vec<u64> = vec![];
    let mut input_b: Vec<u64> = vec![];
    let mut input_c: Vec<u64> = vec![];
    let mut output: Vec<u64> = vec![];
    for _ in 0..1000 {
        let a = rng.next_u64() & 1;
        let b = rng.next_u64() & 1;
        let c = rng.next_u64() & 1;
        input_a.push(a);
        input_b.push(b);
        input_c.push(c);
        output.push(op(a, b, c));
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
            ChallengePort {
                label: "C",
                scale: Scale::ONE,
                values: input_c,
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

fn two_tick_delay_challenge(rng: &mut impl RngCore) -> Challenge {
    let mut input: Vec<u64> = vec![];
    let mut output: Vec<u64> = vec![];
    for tick in 0..1000 {
        let value = rng.next_u64() & 1;
        input.push(value);
        output.push(if tick >= 2 { input[tick - 2] } else { 0 });
    }
    Challenge {
        goal: "Build a circuit where OUT repeats IN exactly two ticks later.".to_string(),
        inputs: vec![ChallengePort {
            label: "IN",
            scale: Scale::ONE,
            values: input,
        }],
        outputs: vec![ChallengePort {
            label: "OUT",
            scale: Scale::ONE,
            values: output,
        }],
        ticks: 1000,
    }
}

fn flip_one_bit_challenge(rng: &mut impl RngCore) -> Challenge {
    let scale = Scale::new(2).expect("flip-one-bit width is a valid scale");
    let mut input: Vec<u64> = vec![];
    let mut output: Vec<u64> = vec![];
    for _ in 0..1000 {
        let value = rng.next_u64() & 0b11;
        input.push(value);
        output.push(value ^ 0b10);
    }
    Challenge {
        goal:
            "Build a circuit where OUT inverts VALUE's top bit and leaves the bottom bit unchanged."
                .to_string(),
        inputs: vec![ChallengePort {
            label: "VALUE",
            scale,
            values: input,
        }],
        outputs: vec![ChallengePort {
            label: "VALUE",
            scale,
            values: output,
        }],
        ticks: 1000,
    }
}

fn memory_cell_challenge(rng: &mut impl RngCore) -> Challenge {
    let value_scale = Scale::new(8).expect("memory cell width is a valid scale");
    let mut input_value: Vec<u64> = vec![];
    let mut set: Vec<u64> = vec![];
    let mut get: Vec<u64> = vec![];
    let mut output_value: Vec<u64> = vec![];
    let mut stored = 0u64;
    for _ in 0..1000 {
        let value = rng.next_u64() & 0xff;
        let set_value = rng.next_u64() & 1;
        let get_value = rng.next_u64() & 1;
        input_value.push(value);
        set.push(set_value);
        get.push(get_value);
        output_value.push(if get_value == 1 { stored } else { 0 });
        if set_value == 1 {
            stored = value;
        }
    }
    Challenge {
        goal: "Build a memory cell: SET stores VALUE for later, GET outputs the stored \
            VALUE, neither outputs 0, and SET plus GET outputs the old value before \
            storing the new one."
            .to_string(),
        inputs: vec![
            ChallengePort {
                label: "VALUE",
                scale: value_scale,
                values: input_value,
            },
            ChallengePort {
                label: "SET",
                scale: Scale::ONE,
                values: set,
            },
            ChallengePort {
                label: "GET",
                scale: Scale::ONE,
                values: get,
            },
        ],
        outputs: vec![ChallengePort {
            label: "VALUE",
            scale: value_scale,
            values: output_value,
        }],
        ticks: 1000,
    }
}

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
                label: "CARRY",
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
                label: "CARRY",
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
        ChallengeId::And3 => ternary_gate_challenge(
            &mut rng,
            "Build a circuit where OUT is on only when A, B, and C are all on.",
            |a, b, c| a & b & c,
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
        ChallengeId::TwoTickDelay => two_tick_delay_challenge(&mut rng),
        ChallengeId::FlipOneBit => flip_one_bit_challenge(&mut rng),
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
        ChallengeId::MemoryCell => memory_cell_challenge(&mut rng),
        ChallengeId::BinaryAddition => Challenge {
            goal: "Fill in the carry bits and output bits for longhand binary addition."
                .to_string(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            ticks: 0,
        },
    }
}

pub const CHALLENGES: [ChallengeId; 16] = [
    ChallengeId::Nor,
    ChallengeId::Or,
    ChallengeId::Nand,
    ChallengeId::And,
    ChallengeId::And3,
    ChallengeId::Xor,
    ChallengeId::Transistor,
    ChallengeId::TwoTickDelay,
    ChallengeId::FlipOneBit,
    ChallengeId::Adder,
    ChallengeId::Adder2,
    ChallengeId::Adder4,
    ChallengeId::SrLatch,
    ChallengeId::TFlipFlop,
    ChallengeId::MemoryCell,
    ChallengeId::BinaryAddition,
];

#[cfg(test)]
mod tests;
