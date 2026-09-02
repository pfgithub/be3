use super::*;

#[test]
fn challenge_port_slots_map_each_port_to_its_dense_slot() {
    let ids = [
        InputId::from_u128(2),
        InputId::from_u128(0),
        InputId::from_u128(2),
    ];

    let slots = challenge_port_slots(ids.into_iter(), 3, InputId::from_u128);

    assert_eq!(slots, vec![Some(0), None, Some(1)]);
}
