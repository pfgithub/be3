use super::*;

#[test]
fn editing_resets_challenge_test_results() {
    let mut editor = nor_challenge_editor();
    editor.challenge_test_run_all();
    assert_eq!(
        editor.challenge.as_ref().unwrap().test.next_tick,
        editor.challenge.as_ref().unwrap().data.ticks
    );

    // Any grid edit must invalidate the compiled test and clear its results.
    editor
        .grid
        .add_component(Point::new(20, 20), Rotation::Up, ComponentKind::Led);
    editor.ensure_challenge_test();

    let test = &editor.challenge.as_ref().unwrap().test;
    assert_eq!(test.next_tick, 0);
    assert!(test.actual.iter().all(|values| values.is_empty()));
}
