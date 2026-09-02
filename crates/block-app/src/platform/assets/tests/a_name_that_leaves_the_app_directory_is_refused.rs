use super::*;

#[test]
fn a_name_that_leaves_the_app_directory_is_refused() {
    for name in [
        "",
        ".",
        "..",
        "../games.json",
        "games/../../secrets",
        "/etc/passwd",
        "games\\connect_four.wasm",
        "C:games.json",
        "games//connect_four.wasm",
    ] {
        assert!(!inside_the_app(name), "{name} should be refused");
    }
}
