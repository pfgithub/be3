use super::{pick_free_color, PresenceColor};

#[test]
fn pick_free_color_avoids_colors_in_use() {
    let used = [
        PresenceColor::Red,
        PresenceColor::Orange,
        PresenceColor::Yellow,
    ];
    let picked = pick_free_color(used);
    assert!(!used.contains(&picked));
}
