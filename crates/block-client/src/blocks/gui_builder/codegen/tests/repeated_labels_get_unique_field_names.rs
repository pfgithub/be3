use super::*;

#[test]
fn repeated_labels_get_unique_field_names() {
    let mut builder = GuiBuilder::new();
    for _ in 0..2 {
        push(
            &mut builder,
            GuiWidgetKind::TextField {
                label: "Name!".into(),
                value: String::new(),
                multiline: false,
            },
        );
    }
    // An unlabelled widget still needs a name, and a keyword-like label
    // cannot be used verbatim.
    push(
        &mut builder,
        GuiWidgetKind::Checkbox {
            label: String::new(),
            checked: false,
        },
    );
    push(
        &mut builder,
        GuiWidgetKind::Checkbox {
            label: "type".into(),
            checked: false,
        },
    );

    let code = builder.generate_code(None);
    assert!(code.contains("pub name: String,"), "{code}");
    assert!(code.contains("pub name_2: String,"), "{code}");
    assert!(code.contains("pub checked: bool,"), "{code}");
    assert!(code.contains("pub type_: bool,"), "{code}");
}
