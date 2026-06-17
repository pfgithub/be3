use super::*;

#[test]
fn call_clears_memory_writes_inputs_and_outputs_through_bindings() {
    let child = component(2, Vec::new(), vec![0], vec![0], Vec::new());
    let root = component_with_children(
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![child],
        vec![Instruction::Call {
            component: 0,
            instance: 0,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![Some(0)],
            outputs: vec![Some(1)],
        }],
    );
    let mut vm = vm_with_root(root);
    vm.memory_stack[0] = 5;

    vm.execute();
    assert_eq!(vm.root_memory(), &[5, 5]);

    vm.begin_tick();
    vm.execute();
    assert_eq!(vm.root_memory(), &[0, 0]);
}
