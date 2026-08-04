use block::Block;

use super::*;
use crate::BlockClient;

#[test]
fn compiled_logic_replace_swaps_the_whole_program() {
    let client = BlockClient::new(Uuid::new_v4(), Uuid::new_v4());
    let source = Uuid::new_v4();
    let block = client.create_block(compiled(source, Vec::new()));
    let called = Uuid::new_v4();

    block.operate(CompiledLogicOperation::Replace {
        compiled: compiled(source, vec![called]),
    });

    let program = block.read().unwrap();
    assert_eq!(program.source(), source);
    assert_eq!(program.calls(), [called]);
    assert_eq!(program.references(), vec![called]);
}
