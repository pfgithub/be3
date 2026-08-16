use super::super::GuestModule;
use super::WASM_DEMO_BYTES;

#[test]
fn run_frame_draws_three_triangles_that_move_over_time() {
    let mut module = GuestModule::load(WASM_DEMO_BYTES).expect("wasm-demo loads");

    let first = module.run_frame(0.0).expect("frame runs");
    assert_eq!(first.vertices.len(), 9);
    assert_ne!(first.clear_color, [0.0, 0.0, 0.0]);

    let second = module.run_frame(1.0).expect("frame runs");
    assert_eq!(second.vertices.len(), 9);
    assert_ne!(first.vertices, second.vertices);
}
