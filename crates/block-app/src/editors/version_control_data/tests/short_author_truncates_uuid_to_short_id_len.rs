use super::*;

#[test]
fn short_author_truncates_uuid_to_short_id_len() {
    let author = uuid::Uuid::from_u128(0xabcdef1234567890abcdef1234567890);
    assert_eq!(short_author(author), "abcdef12");
}
