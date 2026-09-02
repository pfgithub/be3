use super::*;

#[test]
fn a_relative_path_inside_the_app_is_accepted() {
    for name in ["games.json", "games/connect_four.wasm", ".hidden"] {
        assert!(inside_the_app(name), "{name} should be accepted");
    }
}
