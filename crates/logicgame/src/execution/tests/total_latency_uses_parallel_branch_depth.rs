use super::*;

#[test]
fn total_latency_uses_parallel_branch_depth() {
    let root = component(
        4,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            Instruction::Not {
                input: 0,
                output: 2,
            },
            Instruction::Not {
                input: 1,
                output: 3,
            },
        ],
    );

    assert_eq!(root.total_latency(), 1);
}
