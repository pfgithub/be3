use super::*;

#[test]
fn properties_round_trip_recovers_the_original_values() {
    let name_key = Uuid::from_u128(1);
    let other_key = Uuid::from_u128(2);
    let properties = BTreeMap::from([(name_key, b"hello".to_vec()), (other_key, Vec::new())]);

    let encoded = encode_properties(&properties);
    assert_ne!(encoded.get(&name_key), properties.get(&name_key));
    assert_eq!(decode_properties(encoded), properties);
}
