use super::*;

#[test]
fn simulation_tracks_the_next_instruction() {
    let mut editor = LogicEditor::default();
    editor.simulation.vm = Some(Vm::from_unlinked_component(std::rc::Rc::new(
        ExecutionComponent {
            memory_size: 2,
            storage_init: vec![7],
            inputs: Vec::new(),
            outputs: Vec::new(),
            components: Vec::new(),
            instructions: vec![
                Instruction::ReadStorage {
                    storage: 0,
                    output: 0,
                },
                Instruction::Not {
                    input: 0,
                    output: 1,
                },
            ],
            subgraphs: Vec::new(),
            source_hash: None,
        },
    )));

    assert!(editor.begin_simulation_tick());
    assert_eq!(
        simulation_instruction_view(
            editor.simulation.vm.as_ref().unwrap(),
            &editor.simulation.instruction_selection,
            editor.simulation.tick_in_progress
        )
        .next_instruction,
        Some(0)
    );
    editor.execute_next_simulation_instruction();
    assert_eq!(
        simulation_instruction_view(
            editor.simulation.vm.as_ref().unwrap(),
            &editor.simulation.instruction_selection,
            editor.simulation.tick_in_progress
        )
        .next_instruction,
        Some(1)
    );
    assert_eq!(editor.simulation.steps, 0);
    assert!(editor.simulation.tick_in_progress);
    assert_eq!(
        editor.simulation.vm.as_ref().unwrap().root_memory(),
        &[7, 0]
    );

    editor.execute_next_simulation_instruction();
    assert_eq!(
        simulation_instruction_view(
            editor.simulation.vm.as_ref().unwrap(),
            &editor.simulation.instruction_selection,
            editor.simulation.tick_in_progress
        )
        .next_instruction,
        None
    );
    assert_eq!(editor.simulation.steps, 1);
    assert!(!editor.simulation.tick_in_progress);
    assert_eq!(
        editor.simulation.vm.as_ref().unwrap().root_memory(),
        &[7, !7]
    );
}
