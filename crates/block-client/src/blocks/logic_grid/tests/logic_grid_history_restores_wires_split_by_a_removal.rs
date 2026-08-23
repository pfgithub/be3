use super::*;

#[test]
fn logic_grid_history_restores_wires_split_by_a_removal() {
    let (_client, block) = client_with_grid();
    block.operate(LogicGridOperation::AddWire {
        wire: wire((0, 0), (8, 0)),
    });
    let laid = block.read().unwrap().grid().wires().to_vec();
    assert_eq!(laid, vec![wire((0, 0), (8, 0))]);

                                                                               
                                                           
    block.operate(LogicGridOperation::RemoveWire {
        wire: wire((3, 0), (4, 0)),
    });
    assert_eq!(
        block.read().unwrap().grid().wires(),
        &[wire((0, 0), (2, 0)), wire((5, 0), (8, 0))]
    );

    block.undo();
    assert_eq!(block.read().unwrap().grid().wires(), laid.as_slice());

    block.redo();
    assert_eq!(
        block.read().unwrap().grid().wires(),
        &[wire((0, 0), (2, 0)), wire((5, 0), (8, 0))]
    );
}
