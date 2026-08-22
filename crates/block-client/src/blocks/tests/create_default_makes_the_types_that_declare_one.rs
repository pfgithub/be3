use super::*;

#[test]
fn create_default_makes_the_types_that_declare_one() {
    let client = client();
    let block_type = <checklist::Checklist as Block>::TYPE_ID;
    let handle = create_default(&client, block_type).expect("a checklist has a default");
    assert_eq!(handle.block_type(), block_type);
    assert_eq!(
        client
            .get_block::<checklist::Checklist>(handle.id())
            .read()
            .map(|checklist| checklist.items().len()),
        Some(0)
    );

    assert!(create_default(&client, <image::Image as Block>::TYPE_ID).is_none());
}
