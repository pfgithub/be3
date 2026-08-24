use super::*;

#[test]
fn font_runs_switch_fonts_without_losing_script_information() {
    let run = TextRun {
        value: "Hello 世界 ",
        script: Some(Script::Latin),
    };

    assert_eq!(
        split_font_runs(run, |character| {
            if matches!(character, '世' | '界') {
                Some(1)
            } else {
                Some(0)
            }
        }),
        vec![
            FontRun {
                value: "Hello ",
                script: Some(Script::Latin),
                font_index: 0,
            },
            FontRun {
                value: "世界",
                script: Some(Script::Latin),
                font_index: 1,
            },
            FontRun {
                value: " ",
                script: Some(Script::Latin),
                font_index: 0,
            },
        ]
    );
}
