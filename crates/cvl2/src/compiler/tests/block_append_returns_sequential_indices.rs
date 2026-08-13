use super::*;

#[test]
fn block_append_returns_sequential_indices() {
    let mut block = empty_block();
    let validate = block.validate;
    let idx0 = block_append(&mut block, AnalysisLine::Args { pos: pos_at(0) });
    let idx1 = block_append(&mut block, AnalysisLine::Args { pos: pos_at(1) });
    let idx2 = block_append(&mut block, AnalysisLine::Args { pos: pos_at(2) });
    assert_eq!(idx0, BlockIdx(0, validate));
    assert_eq!(idx1, BlockIdx(1, validate));
    assert_eq!(idx2, BlockIdx(2, validate));
    assert_eq!(block.lines.len(), 3);
}
