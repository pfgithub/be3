use super::*;

#[test]
fn not_reads_and_writes_memory() {
    let mut vm = vm_with_root(component(
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![Instruction::Not {
            input: 0,
            output: 1,
        }],
    ));
    vm.memory_stack[0] = 0x00ff;

    vm.execute();

    assert_eq!(vm.root_memory(), &[0x00ff, !0x00ff]);
}
