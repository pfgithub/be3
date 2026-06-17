use super::*;

#[test]
fn nested_subcomponent_storage_is_owned_by_the_root_vm() {
    let leaf_hash = ComponentHash::new("5".repeat(64)).unwrap();
    let leaf = UnlinkedComponent {
        memory_size: 1,
        storage_init: vec![8],
        inputs: vec![0],
        outputs: Vec::new(),
        components: Vec::new(),
        instructions: vec![Instruction::SaveStorage {
            storage: 0,
            input: 0,
        }],
        subgraphs: vec![UnlinkedSubgraph {
            inputs: vec![0],
            outputs: Vec::new(),
            instructions: vec![Instruction::SaveStorage {
                storage: 0,
                input: 0,
            }],
        }],
    };
    let middle = UnlinkedComponent {
        memory_size: 1,
        storage_init: Vec::new(),
        inputs: vec![0],
        outputs: Vec::new(),
        components: vec![leaf_hash.clone()],
        instructions: vec![Instruction::Call {
            component: 0,
            instance: 0,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![Some(0)],
            outputs: vec![],
        }],
        subgraphs: vec![UnlinkedSubgraph {
            inputs: vec![0],
            outputs: Vec::new(),
            instructions: vec![Instruction::Call {
                component: 0,
                instance: 0,
                subgraph: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![],
            }],
        }],
    };
    let middle = middle
        .link(|hash| -> Result<Rc<Component>, ()> {
            assert_eq!(hash, &leaf_hash);
            leaf.link_with_hash(hash.clone(), |_| panic!("leaf has no child components"))
        })
        .unwrap();
    let root = component_with_children(
        1,
        middle.storage_init.clone(),
        Vec::new(),
        Vec::new(),
        vec![middle],
        vec![Instruction::Call {
            component: 0,
            instance: 0,
            subgraph: 0,
            storage_offset: 0,
            inputs: vec![Some(0)],
            outputs: vec![],
        }],
    );
    let mut vm = vm_with_root(root);
    vm.memory_stack[0] = 13;

    vm.execute();

    assert_eq!(vm.storage, vec![13]);
}
