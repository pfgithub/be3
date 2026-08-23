use block::Block;

use super::*;

#[test]
fn compiled_logic_references_the_blocks_it_calls() {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let program = compiled(Uuid::new_v4(), vec![first, second]);

    assert_eq!(program.calls(), [first, second]);
                                                                                
                                                  
    assert_eq!(program.references(), vec![first, second]);
}
