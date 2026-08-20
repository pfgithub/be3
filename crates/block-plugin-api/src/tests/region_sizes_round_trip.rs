use super::*;

#[test]
fn region_sizes_round_trip() {
    let message = Message::RegionSizes(vec![
        RegionSize {
            screen: ScreenId(1),
            logical_width: 400.0,
            logical_height: 24.5,
        },
        RegionSize {
            screen: ScreenId(2),
            logical_width: 200.0,
            logical_height: 300.0,
        },
    ]);
    assert_eq!(
        decode_frame(&encode_frame(&message).unwrap()).unwrap(),
        message
    );

    let oversized = Message::RegionSizes(vec![
        RegionSize {
            screen: ScreenId(1),
            logical_width: 1.0,
            logical_height: 1.0,
        };
        MAX_COLLECTION_ITEMS + 1
    ]);
    assert_eq!(
        encode_frame(&oversized),
        Err(DecodeError::LimitExceeded("collection"))
    );
}
