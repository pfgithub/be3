use super::*;

#[test]
fn total_latency_includes_subcomponent_latency() {
    let child = component(
        3,
        Vec::new(),
        vec![0],
        vec![2],
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
    let root = component_with_children(
        2,
        Vec::new(),
        vec![0],
        vec![1],
        vec![child],
        vec![
            Instruction::Call {
                component: 0,
                instance: 0,
                subgraph: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![Some(0)],
            },
            Instruction::Not {
                input: 0,
                output: 1,
            },
        ],
    );

    assert_eq!(root.total_latency(), 3);
}
