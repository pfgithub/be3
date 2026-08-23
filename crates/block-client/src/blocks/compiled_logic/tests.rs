use logicgame::{
    execution::{Instruction, UnlinkedComponent, UnlinkedSubgraph},
    grid::{ComponentPort, ComponentSide, Scale, Size},
};
use uuid::Uuid;

use super::{CompiledLogic, CompiledLogicOperation};

                                                                             
                  
fn compiled(source: Uuid, calls: Vec<Uuid>) -> CompiledLogic {
    let instructions = vec![Instruction::Not {
        input: 0,
        output: 1,
    }];
    CompiledLogic::new(
        source,
        Size::new(2, 2),
        vec![
            ComponentPort::input(0, Scale::ONE, ComponentSide::Bottom, 0, 1),
            ComponentPort::output(0, Scale::ONE, ComponentSide::Top, 0, 1),
        ],
        UnlinkedComponent {
            inputs: vec![0],
            outputs: vec![1],
            components: calls,
            instructions: instructions.clone(),
            subgraphs: vec![UnlinkedSubgraph {
                inputs: vec![0],
                outputs: vec![0],
                instructions,
            }],
            memory_size: 2,
            storage_init: Vec::new(),
        },
    )
}

mod compiled_logic_placement_names_its_own_block;
mod compiled_logic_references_the_blocks_it_calls;
mod compiled_logic_replace_swaps_the_whole_program;
mod compiling_a_grid_derives_ports_from_its_inputs_and_outputs;
mod compiling_an_empty_grid_is_refused;
