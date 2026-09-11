use super::*;
use crate::{provide_context, use_context};

#[derive(Clone, PartialEq, Debug)]
struct Width(f32);

#[test]
fn the_nearest_context_shadows_the_ones_above_it() {
    let outer = Scope::new();
    let seen = Rc::new(RefCell::new(Vec::new()));

    outer.run({
        let seen = seen.clone();
        move || {
            provide_context(Width(1024.0));
            let inner = Scope::new();
            inner.run({
                let seen = seen.clone();
                move || {
                    provide_context(Width(320.0));
                    seen.borrow_mut().push(use_context::<Width>());
                }
            });
            seen.borrow_mut().push(use_context::<Width>());
        }
    });

    assert_eq!(
        *seen.borrow(),
        vec![Some(Width(320.0)), Some(Width(1024.0))]
    );
}
