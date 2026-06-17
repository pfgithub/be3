use super::*;

#[test]
fn compiled_component_serializes_unlinked_component_and_source_id() {
    let source_file_id = Uuid::new_v4();
    let compiled = CompiledComponent {
        source_file_id,
        snapshot: LogicGrid::new().snapshot(),
        component: UnlinkedComponent {
            memory_size: 3,
            storage_init: vec![6, 7],
            inputs: vec![2],
            outputs: vec![1],
            components: Vec::new(),
            instructions: vec![Instruction::Not {
                input: 2,
                output: 1,
            }],
            subgraphs: vec![logicgame::execution::UnlinkedSubgraph {
                inputs: vec![0],
                outputs: vec![0],
                instructions: vec![Instruction::Not {
                    input: 2,
                    output: 1,
                }],
            }],
        },
    };

    let json = serde_json::to_value(&compiled).unwrap();
    assert_eq!(json["source_file_id"], source_file_id.to_string());
    assert_eq!(json["component"]["memory_size"], 3);
    assert_eq!(json["component"]["storage_init"], serde_json::json!([6, 7]));
    assert_eq!(json["component"]["inputs"], serde_json::json!([2]));
    assert_eq!(json["component"]["outputs"], serde_json::json!([1]));
    assert!(json["component"]["instructions"].is_array());
    assert!(json["component"]["subgraphs"].is_array());

    let decoded: CompiledComponent = serde_json::from_value(json).unwrap();
    assert_eq!(decoded.source_file_id, compiled.source_file_id);
    assert_eq!(decoded.snapshot, compiled.snapshot);
    assert_eq!(decoded.component, compiled.component);
}
