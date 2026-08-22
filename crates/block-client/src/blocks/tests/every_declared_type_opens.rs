use super::*;

#[test]
fn every_declared_type_opens() {
    let client = client();
    assert!(!TYPE_IDS.is_empty());
    for block_type in TYPE_IDS {
        let id = Uuid::new_v4();
        let handle = open(&client, id, *block_type)
            .unwrap_or_else(|| panic!("block type {block_type} is not in the table"));
        assert_eq!(handle.id(), id);
        assert_eq!(handle.block_type(), *block_type);
    }
}
