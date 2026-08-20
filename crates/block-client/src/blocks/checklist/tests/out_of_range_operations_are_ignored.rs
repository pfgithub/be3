use super::*;

#[test]
fn out_of_range_operations_are_ignored() {
    let mut checklist = Checklist::new();
    Checklist::apply_operation(
        &mut checklist,
        &ChecklistOperation::Add {
            text: "only".to_owned(),
        },
    );
    let expected = checklist.clone();

    for operation in [
        ChecklistOperation::SetText {
            index: 1,
            text: "missing".to_owned(),
        },
        ChecklistOperation::SetDone {
            index: 7,
            done: true,
        },
        ChecklistOperation::Remove { index: 3 },
    ] {
        Checklist::apply_operation(&mut checklist, &operation);
    }

    assert_eq!(checklist, expected);
}
