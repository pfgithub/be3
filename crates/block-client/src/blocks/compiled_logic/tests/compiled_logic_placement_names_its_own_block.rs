use logicgame::grid::ComponentKind;

use super::*;

#[test]
fn compiled_logic_placement_names_its_own_block() {
    let id = Uuid::new_v4();
    let program = compiled(Uuid::new_v4(), Vec::new());

    let kind = program.placement(id, "Half Adder").unwrap();

    let ComponentKind::Subcomponent {
        compiled: called,
        name,
        size,
        ports,
        subgraphs,
        ..
    } = kind
    else {
        panic!("a compiled program places as a subcomponent");
    };
    assert_eq!(called, id);
    assert_eq!(name, "Half Adder");
    assert_eq!(size, program.size());
    assert_eq!(ports, program.ports());
    assert_eq!(subgraphs, program.subgraphs());
}
