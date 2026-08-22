use super::*;

#[test]
fn an_unknown_block_type_has_no_handle() {
    let client = client();
    let unknown = Uuid::new_v4();
    assert!(open(&client, Uuid::new_v4(), unknown).is_none());
    assert!(create_default(&client, unknown).is_none());
}
