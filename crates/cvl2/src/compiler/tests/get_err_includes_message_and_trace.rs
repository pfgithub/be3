use super::*;

#[test]
fn get_err_includes_message_and_trace() {
    let mut env = new_env();
    env.trace.push(crate::parser::TraceEntry {
        pos: pos_at(0),
        text: "caller".to_string(),
    });

    let err = get_err(
        &env,
        Some(pos_at(5)),
        "something broke",
        Some(vec![(Some(pos_at(6)), "extra context".to_string())]),
        None,
    );

    assert_eq!(err.entries.len(), 2);
    assert_eq!(err.entries[0].message, "something broke");
    assert_eq!(err.entries[0].style, ErrorStyle::Error);
    assert_eq!(err.entries[1].message, "extra context");
    assert_eq!(err.entries[1].style, ErrorStyle::Note);
    assert_eq!(err.trace, env.trace);
}
