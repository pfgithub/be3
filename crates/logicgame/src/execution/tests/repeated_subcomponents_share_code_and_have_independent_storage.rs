use super::*;

#[test]
fn repeated_subcomponents_share_code_and_have_independent_storage() {
    let hash = ComponentHash::new("3".repeat(64)).unwrap();
    let leaf_unlinked = UnlinkedComponent {
        memory_size: 1,
        storage_init: vec![0],
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
    let root_unlinked = UnlinkedComponent {
        memory_size: 2,
        storage_init: Vec::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        components: vec![hash.clone()],
        instructions: vec![
            Instruction::Call {
                component: 0,
                instance: 0,
                subgraph: 0,
                storage_offset: 0,
                inputs: vec![Some(0)],
                outputs: vec![],
            },
            Instruction::Call {
                component: 0,
                instance: 1,
                subgraph: 0,
                storage_offset: 0,
                inputs: vec![Some(1)],
                outputs: vec![],
            },
        ],
        subgraphs: Vec::new(),
    };
    let mut cache = BTreeMap::<ComponentHash, Rc<Component>>::new();
    let root = root_unlinked
        .link(|requested| {
            Ok::<_, ()>(
                cache
                    .entry(requested.clone())
                    .or_insert_with(|| {
                        leaf_unlinked
                            .link_with_hash(requested.clone(), |_| -> Result<Rc<Component>, ()> {
                                panic!("leaf has no child components")
                            })
                            .unwrap()
                    })
                    .clone(),
            )
        })
        .unwrap();
    assert_eq!(root.components.len(), 1);

    let mut vm = vm_with_root(root);
    vm.memory_stack.copy_from_slice(&[1, 2]);
    vm.execute();

    assert_eq!(vm.storage, vec![1, 2]);
}
