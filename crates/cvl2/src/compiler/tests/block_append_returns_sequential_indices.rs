use super::*;

#[test]
fn block_append_returns_sequential_indices() {
    let mut block = AnalysisBlock { lines: Vec::new() };
    let idx0 = block_append(&mut block, AnalysisLine::Void { pos: pos_at(0) });
    let idx1 = block_append(&mut block, AnalysisLine::Void { pos: pos_at(1) });
    let idx2 = block_append(&mut block, AnalysisLine::Void { pos: pos_at(2) });
    assert_eq!(idx0, BlockIdx(0));
    assert_eq!(idx1, BlockIdx(1));
    assert_eq!(idx2, BlockIdx(2));
    assert_eq!(block.lines.len(), 3);
}
