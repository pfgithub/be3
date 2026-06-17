use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::{
    challenges::{input_id, output_id, ChallengeComponentKind, NOR_CHALLENGE},
    grid::{InputId, OutputId, Scale},
};

#[test]
fn nor_challenge_defines_ports_and_generator() {
    assert_eq!(NOR_CHALLENGE.inputs.len(), 2);
    assert_eq!(NOR_CHALLENGE.outputs.len(), 1);
    assert_eq!(input_id(&NOR_CHALLENGE, "A"), Some(InputId::from_u128(1)));
    assert_eq!(input_id(&NOR_CHALLENGE, "B"), Some(InputId::from_u128(2)));
    assert_eq!(
        output_id(&NOR_CHALLENGE, "OUT"),
        Some(OutputId::from_u128(1))
    );
    assert!(NOR_CHALLENGE
        .inputs
        .iter()
        .chain(NOR_CHALLENGE.outputs)
        .all(|port| port.scale == Scale::ONE));
    assert_eq!(NOR_CHALLENGE.validation_ticks, 1000);
    assert_eq!(
        NOR_CHALLENGE.disallowed_components,
        &[
            ChallengeComponentKind::MergerSplitter,
            ChallengeComponentKind::Led,
            ChallengeComponentKind::Storage,
            ChallengeComponentKind::Subcomponent,
        ]
    );

    let mut rng = ChaCha8Rng::seed_from_u64(7);
    for _ in 0..64 {
        let tick = (NOR_CHALLENGE.generate)(&mut rng);
        let a = tick
            .inputs
            .iter()
            .find(|value| value.label == "A")
            .unwrap()
            .value;
        let b = tick
            .inputs
            .iter()
            .find(|value| value.label == "B")
            .unwrap()
            .value;
        let out = tick
            .outputs
            .iter()
            .find(|value| value.label == "OUT")
            .unwrap()
            .value;
        assert!(a <= 1);
        assert!(b <= 1);
        assert_eq!(out, u64::from((a | b) == 0));
    }
}
