use super::*;

#[test]
fn script_runs_cover_scripts_from_unicode_data() {
    assert_eq!(
        script_runs("Rust𞤀")
            .into_iter()
            .map(|run| (run.value, run.script))
            .collect::<Vec<_>>(),
        vec![("Rust", Some(Script::Latin)), ("𞤀", Some(Script::Adlam)),]
    );
}
