use super::*;

// Mirrors the TS tokenizer, which throws on a backslash inside a string
// (`throw new Error("TODO impl in_string '\\' char")`) since escape handling
// was never implemented there.
#[test]
#[should_panic(expected = "TODO impl in_string")]
fn tokenize_in_string_backslash_is_unimplemented() {
    let _ = tokenize_str("\"a\\b\"");
}
