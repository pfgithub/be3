use super::*;

#[test]
fn total_latency_counts_save_storage_as_terminal_work() {
    let root = component(
        1,
        vec![0],
        Vec::new(),
        Vec::new(),
        vec![Instruction::SaveStorage {
            storage: 0,
            input: 0,
        }],
    );

    assert_eq!(root.total_latency(), 1);
}
