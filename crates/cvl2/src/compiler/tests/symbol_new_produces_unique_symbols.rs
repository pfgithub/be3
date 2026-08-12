use super::*;

#[test]
fn symbol_new_produces_unique_symbols() {
    let a = Symbol::new();
    let b = Symbol::new();
    let c = Symbol::default();
    assert_ne!(a, b);
    assert_ne!(b, c);
    assert_ne!(a, c);
    assert_eq!(a, a);
}
