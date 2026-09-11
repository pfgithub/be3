use super::*;
use crate::{provide_context, use_context};

#[derive(Clone, PartialEq, Debug)]
struct Width(f32);

#[test]
fn a_context_reaches_the_effects_a_nested_scope_creates() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let scope = Scope::new();
    let (count, set_count) = create_signal(0);

    scope.run({
        let seen = seen.clone();
        move || {
            provide_context(Width(320.0));
            assert_eq!(use_context::<Width>(), Some(Width(320.0)));
            create_effect(move || {
                count.get();
                seen.borrow_mut().push(use_context::<Width>());
            });
        }
    });

    set_count.set(1);

    assert_eq!(*seen.borrow(), vec![Some(Width(320.0)), Some(Width(320.0))]);
    assert_eq!(use_context::<Width>(), None);
}
