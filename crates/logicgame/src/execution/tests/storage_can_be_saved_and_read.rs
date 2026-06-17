use super::*;

#[test]
fn storage_can_be_saved_and_read() {
    let mut vm = vm_with_root(component(
        2,
        vec![0],
        Vec::new(),
        Vec::new(),
        vec![
            Instruction::SaveStorage {
                storage: 0,
                input: 0,
            },
            Instruction::ReadStorage {
                storage: 0,
                output: 1,
            },
        ],
    ));
    vm.memory_stack[0] = 42;

    vm.execute();

    assert_eq!(vm.storage, vec![42]);
    assert_eq!(vm.root_memory(), &[42, 42]);
}
