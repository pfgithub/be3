use block_client::block_url;

use super::*;

#[test]
fn replace_block_reference_replaces_every_matching_url() {
    let workspace = Uuid::from_u128(1);
    let other_workspace = Uuid::from_u128(2);
    let old = Uuid::from_u128(3);
    let new = Uuid::from_u128(4);
    let text = format!(
        "{} {} {}",
        block_url(workspace, old),
        block_url(other_workspace, old),
        block_url(workspace, Uuid::from_u128(5)),
    );
    let mut tester = EditorTester::new(text);

    tester.execute(EditorCommand::ReplaceBlockReference { old, new });

    tester.expect_content(format!(
        "{}| {} {}",
        block_url(workspace, new),
        block_url(other_workspace, new),
        block_url(workspace, Uuid::from_u128(5)),
    ));
}
