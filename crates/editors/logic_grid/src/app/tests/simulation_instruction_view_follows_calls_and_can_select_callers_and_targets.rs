use super::*;

#[test]
fn simulation_instruction_view_follows_calls_and_can_select_callers_and_targets() {
    let child = Rc::new(ExecutionComponent {
        memory_size: 2,
        storage_init: Vec::new(),
        inputs: vec![0],
        outputs: vec![1],
        components: Vec::new(),
        instructions: vec![Instruction::Not {
            input: 0,
            output: 1,
        }],
        subgraphs: vec![ComponentExecutionSubgraph {
            inputs: vec![0],
            outputs: vec![0],
            instructions: vec![Instruction::Not {
                input: 0,
                output: 1,
            }],
        }],
        source: None,
    });
    let root = Rc::new(ExecutionComponent {
        memory_size: 2,
        storage_init: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        components: vec![Rc::clone(&child)],
        instructions: vec![Instruction::Call {
            component: 0,
            instance: 0,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![Some(0)],
            outputs: vec![Some(1)],
        }],
        subgraphs: Vec::new(),
        source: None,
    });
    let mut editor = LogicGridEditor::default();
    editor.simulation.vm = Some(Vm::from_unlinked_component(Rc::clone(&root)));

    assert!(editor.begin_simulation_tick());
    editor.execute_next_simulation_instruction();

    let vm = editor.simulation.vm.as_ref().unwrap();
    let active = simulation_instruction_view(
        vm,
        &SimulationInstructionSelection::Active,
        editor.simulation.tick_in_progress,
    );
    assert!(Rc::ptr_eq(active.component, &child));
    assert_eq!(active.next_instruction, Some(0));

    let caller = simulation_instruction_view(
        vm,
        &SimulationInstructionSelection::ReturnFrame(0),
        editor.simulation.tick_in_progress,
    );
    assert!(Rc::ptr_eq(caller.component, &root));
    assert_eq!(caller.next_instruction, Some(0));

    let target_selection = SimulationInstructionSelection::Component(Rc::clone(&child));
    let target =
        simulation_instruction_view(vm, &target_selection, editor.simulation.tick_in_progress);
    assert!(Rc::ptr_eq(target.component, &child));
    assert_eq!(target.next_instruction, None);
}
