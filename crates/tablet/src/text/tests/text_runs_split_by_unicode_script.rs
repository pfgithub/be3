use super::*;

#[test]
fn text_runs_split_by_unicode_script() {
    assert_eq!(
        script_runs("Hello 世界 مرحبا")
            .into_iter()
            .map(|run| (run.value, run.script))
            .collect::<Vec<_>>(),
        vec![
            ("Hello ", Some(Script::Latin)),
            ("世界 ", Some(Script::Han)),
            ("مرحبا", Some(Script::Arabic)),
        ]
    );
}
