use super::{pick_free_color, PresenceColor};

#[test]
fn pick_free_color_falls_back_to_a_palette_color_when_all_are_used() {
    let picked = pick_free_color(PresenceColor::ALL);
    assert!(PresenceColor::ALL.contains(&picked));
}
