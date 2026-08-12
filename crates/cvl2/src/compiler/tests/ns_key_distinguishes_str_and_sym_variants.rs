use super::*;

#[test]
fn ns_key_distinguishes_str_and_sym_variants() {
    let sym_a = Symbol::new();
    let sym_b = Symbol::new();
    let mut map: HashMap<NsKey, &str> = HashMap::new();
    map.insert(NsKey::Str("main".to_string()), "string entry");
    map.insert(NsKey::Sym(sym_a), "sym a entry");
    map.insert(NsKey::Sym(sym_b), "sym b entry");

    assert_eq!(
        map.get(&NsKey::Str("main".to_string())),
        Some(&"string entry")
    );
    assert_eq!(map.get(&NsKey::Sym(sym_a)), Some(&"sym a entry"));
    assert_eq!(map.get(&NsKey::Sym(sym_b)), Some(&"sym b entry"));
    assert_eq!(map.get(&NsKey::Str("other".to_string())), None);
    assert_eq!(map.len(), 3);
}
