use super::*;

#[test]
fn read_destructure_empty_input_reports_error() {
    let mut env = new_env();
    let Err(err) = read_destructure(&mut env, pos_at(0), &[]) else {
        panic!("expected an error");
    };

    assert_eq!(
        err.e.entries[0].message,
        "Expected at least one item to destructure"
    );
}
