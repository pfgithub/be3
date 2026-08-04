use super::*;

#[test]
#[should_panic(expected = "is not loaded")]
fn unloaded_call_panics() {
    let missing = Uuid::from_u128(0);
    let mut vm = vm_with_root(component_with_children(
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![Rc::new(Component::unresolved(missing))],
        vec![Instruction::Call {
            component: 0,
            instance: 0,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![],
            outputs: vec![],
        }],
    ));

    vm.execute();
}
