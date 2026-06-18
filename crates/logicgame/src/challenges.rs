use crate::grid::Scale;
use rand::RngCore;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ChallengeId {
    Nor,
}

impl ChallengeId {
    pub fn name(self) -> &'static str {
        match self {
            ChallengeId::Nor => "NOR",
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

pub fn generate_challenge(challenge: ChallengeId) -> Challenge {
    use rand::SeedableRng;
    let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(0);

    match challenge {
        ChallengeId::Nor => {
            let mut input_a: Vec<u64> = vec![];
            let mut input_b: Vec<u64> = vec![];
            let mut output: Vec<u64> = vec![];
            for _ in 0..1000 {
                let a = rng.next_u64() & 1;
                let b = rng.next_u64() & 1;
                let out = u64::from((a | b) == 0);
                input_a.push(a);
                input_b.push(b);
                output.push(out);
            }
            Challenge {
                goal: "Build a circuit where OUT is on only when A and B are both off.".to_string(),
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
    }
}

pub const CHALLENGES: [ChallengeId; 1] = [ChallengeId::Nor];
