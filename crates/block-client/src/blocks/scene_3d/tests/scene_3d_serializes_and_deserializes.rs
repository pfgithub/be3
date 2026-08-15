use super::Scene3D;

#[test]
fn scene_3d_serializes_and_deserializes() {
    let scene = Scene3D::new();

    let encoded = serde_json::to_vec(&scene).unwrap();
    let decoded: Scene3D = serde_json::from_slice(&encoded).unwrap();

    assert_eq!(decoded, scene);
}
