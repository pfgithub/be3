use super::*;

#[test]
fn block_values_deduplicate_and_rewrite_direct_references() {
    let schema = Uuid::new_v4();
    let old = Uuid::new_v4();
    let new = Uuid::new_v4();
    let repo_relative = BlockRef::RepoRelative {
        repo: Uuid::new_v4(),
        eternal_id: Uuid::new_v4(),
    };
    let fields = [Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];
    let mut database = Database::new(BlockRef::Direct(schema));
    for (row_index, value) in [
        BlockRef::Direct(old),
        BlockRef::Direct(old),
        repo_relative,
    ]
    .into_iter()
    .enumerate()
    {
        Database::apply_operation(
            &mut database,
            &DatabaseOperation::SetCell {
                row_index,
                field_id: fields[row_index],
                value: Some(DatabaseValue::Block(value)),
            },
        );
    }
    assert_eq!(database.references(), vec![schema, old]);

    for operation in database.replace_child(old, new).unwrap() {
        Database::apply_operation(&mut database, &operation);
    }
    assert_eq!(database.references(), vec![schema, new]);
    assert_eq!(
        database.rows()[0].value(fields[0]),
        Some(&DatabaseValue::Block(BlockRef::Direct(new)))
    );
    assert_eq!(
        database.rows()[2].value(fields[2]),
        Some(&DatabaseValue::Block(repo_relative))
    );

    for operation in database.delete_child(new).unwrap() {
        Database::apply_operation(&mut database, &operation);
    }
    assert_eq!(database.references(), vec![schema]);
    assert_eq!(database.rows()[0].value(fields[0]), None);
    assert_eq!(database.rows()[1].value(fields[1]), None);
}
