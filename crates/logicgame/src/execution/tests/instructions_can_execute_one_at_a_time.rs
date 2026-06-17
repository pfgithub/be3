use super::*;

#[test]
fn instructions_can_execute_one_at_a_time() {
    let mut vm = vm_with_root(component(
        2,
        vec![7],
        Vec::new(),
        Vec::new(),
        vec![
            Instruction::ReadStorage {
                storage: 0,
                output: 0,
            },
            Instruction::Not {
                input: 0,
                output: 1,
            },
        ],
    ));

    vm.execute_instruction();
    assert_eq!(vm.root_memory(), &[7, 0]);

    vm.execute_instruction();
    assert_eq!(vm.root_memory(), &[7, !7]);
}
