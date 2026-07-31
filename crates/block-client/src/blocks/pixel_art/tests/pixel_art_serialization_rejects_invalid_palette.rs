use super::{PixelArt, PixelColor, MAX_PIXEL_ART_PALETTE_COLORS};

#[test]
fn pixel_art_serialization_rejects_invalid_palette() {
    let art = PixelArt::new();
    let mut value = serde_json::to_value(&art).unwrap();
    value["palette"] = serde_json::to_value(
        (0..=MAX_PIXEL_ART_PALETTE_COLORS)
            .map(|red| PixelColor::new(red as u8, 2, 3, 4))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    assert!(serde_json::from_value::<PixelArt>(value.clone()).is_err());

    value["palette"] =
        serde_json::to_value([PixelColor::new(1, 2, 3, 4), PixelColor::new(1, 2, 3, 4)]).unwrap();
    assert!(serde_json::from_value::<PixelArt>(value).is_err());
}
