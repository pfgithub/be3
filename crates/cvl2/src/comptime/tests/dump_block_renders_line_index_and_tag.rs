use super::*;

#[test]
fn dump_block_renders_line_index_and_tag() {
    let block = AnalysisBlock {
        lines: vec![AnalysisLine::ComptimeOnly { pos: pos_at(0) }],
    };

    let dump = dump_block(&block, usize::MAX);

    assert!(dump.contains("0 = "));
    assert!(dump.contains("comptime:only"));
}
