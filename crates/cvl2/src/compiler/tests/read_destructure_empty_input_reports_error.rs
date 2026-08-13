use super::*;

#[test]
fn read_destructure_empty_input_reports_error() {
    let mut env = new_env();
    let mut targets = Vec::new();
    let Err(err) = read_destructure(&mut env, pos_at(0), &[], &mut targets) else {
        panic!("expected an error");
    };
    let PositionedError::Fresh(e) = err else {
        panic!("expected a fresh error");
    };

    assert!(e.entries[0]
        .message
        .starts_with("Expected at least one item to destructure"));
}
