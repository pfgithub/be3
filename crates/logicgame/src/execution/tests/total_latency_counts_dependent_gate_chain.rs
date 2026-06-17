use super::*;

#[test]
fn total_latency_counts_dependent_gate_chain() {
    let root = component(
        3,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            Instruction::Not {
                input: 0,
                output: 1,
            },
            Instruction::Not {
                input: 1,
                output: 2,
            },
        ],
    );

    assert_eq!(root.total_latency(), 2);
}
