use super::*;

#[test]
fn round_trip_handles_empty_input() {
    let plain: Vec<u8> = Vec::new();
    let encoded = encode(&plain);
    assert_eq!(decode(&encoded), plain);
}
