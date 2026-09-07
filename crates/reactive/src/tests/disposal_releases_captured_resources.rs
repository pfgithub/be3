use super::*;

#[test]
fn disposal_releases_captured_resources() {
    let scope = Scope::new();
    let resource = Rc::new(());
    let weak = Rc::downgrade(&resource);
    let effect = scope.run(|| {
        create_effect(move || {
            let _ = &resource;
        })
    });
    assert!(weak.upgrade().is_some());
    effect.dispose();
    assert!(weak.upgrade().is_none());
    assert!(effect.is_disposed());
}
