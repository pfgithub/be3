use super::*;

#[test]
fn an_unknown_pass_handle_is_reported() {
    let mut gpu = gpu();
    gpu.draw(404, 0, 3, 0, 1);
    let error = gpu.take_error().expect("the unknown pass should report");
    assert!(error.contains("render pass"), "{error}");
}
