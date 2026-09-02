use super::*;

#[test]
fn instructions_have_compact_display_names() {
    assert_eq!(
        format_instruction(&Instruction::Not {
            input: 2,
            output: 5,
        }),
        "NOT m2 -> m5"
    );
    assert_eq!(
        format_instruction(&Instruction::ReadStorage {
            storage: 3,
            output: 4,
        }),
        "READ s3 -> m4"
    );
    assert_eq!(
        format_instruction(&Instruction::SaveStorage {
            storage: 3,
            input: 4,
        }),
        "SAVE m4 -> s3"
    );
}
