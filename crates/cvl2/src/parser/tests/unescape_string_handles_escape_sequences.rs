use super::*;
use crate::compiler::{ComptimeScopeMap, Env, PerComptimeScopeCache, Scope};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

fn empty_env() -> Env {
    Env {
        trace: Vec::new(),
        errors: Vec::new(),
        scope: Scope {
            comptime: ComptimeScopeMap::root(HashMap::new()),
            bindings: Rc::new(RefCell::new(HashMap::new())),
        },
        fn_cache: Rc::new(PerComptimeScopeCache::new()),
        decl_cache: Rc::new(PerComptimeScopeCache::new()),
        builtin_cache: Rc::new(PerComptimeScopeCache::new()),
    }
}

fn pos() -> TokenPosition {
    TokenPosition {
        fyl: "test.cvl2".to_string(),
        idx: 0,
        lyn: 1,
        col: 1,
    }
}

#[test]
fn unescape_string_handles_escape_sequences() {
    let env = empty_env();

    assert_eq!(unescape_string(&env, "hello", pos()).unwrap(), "hello");
    assert_eq!(unescape_string(&env, r"a\nb", pos()).unwrap(), "a\nb");
    assert_eq!(unescape_string(&env, r"a\rb", pos()).unwrap(), "a\rb");
    assert_eq!(unescape_string(&env, r#"a\"b"#, pos()).unwrap(), "a\"b");
    assert_eq!(unescape_string(&env, r"a\'b", pos()).unwrap(), "a'b");
    assert_eq!(unescape_string(&env, r"a\x41b", pos()).unwrap(), "aAb");

    assert!(unescape_string(&env, r"a\qb", pos()).is_err());
    assert!(unescape_string(&env, "a\nb\\c", pos()).is_err());
}
