use super::*;

#[test]
fn total_instruction_count_includes_called_components_per_call() {
    let child = component(
        1,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        (0..10)
            .map(|_| Instruction::Not {
                input: 0,
                output: 0,
            })
            .collect(),
    );
    let root = component_with_children(
        1,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![child],
        vec![
            Instruction::Not {
                input: 0,
                output: 0,
            },
            Instruction::Not {
                input: 0,
                output: 0,
            },
            Instruction::Call {
                component: 0,
                instance: 0,
                subgraph: 0,
                storage_offset: 0,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
            Instruction::Call {
                component: 0,
                instance: 1,
                subgraph: 0,
                storage_offset: 0,
                inputs: Vec::new(),
                outputs: Vec::new(),
            },
            Instruction::Not {
                input: 0,
                output: 0,
            },
        ],
    );

    assert_eq!(root.total_instruction_count(), 25);
}
