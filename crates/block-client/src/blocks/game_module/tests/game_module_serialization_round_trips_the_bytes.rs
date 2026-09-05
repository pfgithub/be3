use super::{wasm_bytes, GameModule};

#[test]
fn game_module_serialization_round_trips_the_bytes() {
    let bytes = wasm_bytes();
    let module = GameModule::new("crazy_8s.wasm", bytes.clone());

    let json = serde_json::to_string(&module).unwrap();
    let decoded: GameModule = serde_json::from_str(&json).unwrap();

    assert!(!json.contains("[0,97,115,109"));
    assert_eq!(decoded, module);
    assert_eq!(decoded.data(), bytes);
}
