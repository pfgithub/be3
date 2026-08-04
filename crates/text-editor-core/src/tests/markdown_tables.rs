use super::*;

#[test]
fn markdown_tables() {
    let source = b"before\n| Left | Center | Right |\n| :--- | :----: | ----: |\nno pipe | *middle* | escaped \\| pipe\n\nafter\n";
    let mut tester = EditorTester::with_language(source, TextLanguage::Markdown);
    let highlight = tester.editor.highlight();
    let tables = highlight.markdown_tables();

    assert_eq!(tables.len(), 1);
    let table = &tables[0];
    assert_eq!(
        table.alignments,
        [
            MarkdownTableAlignment::Left,
            MarkdownTableAlignment::Center,
            MarkdownTableAlignment::Right,
        ]
    );
    assert_eq!(table.rows.len(), 3);
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| &source[row.range.clone()])
            .collect::<Vec<_>>(),
        [
            b"| Left | Center | Right |".as_slice(),
            b"| :--- | :----: | ----: |".as_slice(),
            b"no pipe | *middle* | escaped \\| pipe".as_slice(),
        ]
    );
    assert_eq!(
        table
            .rows
            .iter()
            .map(|row| {
                row.cells
                    .iter()
                    .map(|cell| &source[cell.clone()])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>(),
        [
            vec![
                b"Left ".as_slice(),
                b"Center ".as_slice(),
                b"Right ".as_slice(),
            ],
            vec![
                b":---".as_slice(),
                b":----:".as_slice(),
                b"----:".as_slice()
            ],
            vec![
                b"no pipe ".as_slice(),
                b"*middle* ".as_slice(),
                b"escaped \\| pipe".as_slice(),
            ],
        ]
    );
}
