use super::*;

#[test]
fn database_references_its_schema() {
    let schema_id = Uuid::new_v4();
    let database = Database::new(schema_id);

    assert_eq!(database.references(), vec![schema_id]);
}
