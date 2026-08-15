use super::*;

#[test]
fn round_trip_recovers_the_original_bytes() {
    let plain = b"{\"value\":{\"count\":42}}".repeat(8);
    let encoded = encode(&plain);
    assert_ne!(encoded, plain);
    assert_eq!(decode(&encoded), plain);
}
