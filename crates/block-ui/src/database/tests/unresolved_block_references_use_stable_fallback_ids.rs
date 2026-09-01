use super::*;

#[test]
fn unresolved_block_references_use_stable_fallback_ids() {
    let field = field(DatabaseFieldType::Block);
    let direct = Uuid::new_v4();
    assert_eq!(
        database_value_text(
            &DatabaseValue::Block(BlockRef::Direct(direct)),
            &field,
            &HashMap::new()
        ),
        direct.to_string()
    );
    let eternal_id = Uuid::new_v4();
    assert_eq!(
        database_value_text(
            &DatabaseValue::Block(BlockRef::RepoRelative {
                repo: Uuid::new_v4(),
                eternal_id,
            }),
            &field,
            &HashMap::new()
        ),
        eternal_id.to_string()
    );
}
