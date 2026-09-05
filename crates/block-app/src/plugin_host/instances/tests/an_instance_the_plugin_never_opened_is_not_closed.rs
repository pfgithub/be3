use super::*;

#[test]
fn an_instance_the_plugin_never_opened_is_not_closed() {
    let (mut instances, ..) = placed();

    assert!(!instances.remove(INSTANCE));

    let (mut instances, ..) = placed();
    instances.next_screens(PASS);

    assert!(instances.remove(INSTANCE));
}
