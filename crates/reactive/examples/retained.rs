use std::cell::RefCell;
use std::rc::Rc;

use reactive::{batch, create_effect, create_memo, create_signal, Scope};

#[derive(Default)]
struct Label {
    text: String,
}

fn main() {
    let scope = Scope::new();
    let label = Rc::new(RefCell::new(Label::default()));
    let (count, set_count) = create_signal(0);

    scope.run(|| {
        let text = create_memo(move || format!("Count: {}", count.get()));
        let label = label.clone();
        create_effect(move || label.borrow_mut().text = text.get());
    });

    batch(|| {
        let _label = label.borrow_mut();
        set_count.set(1);
        set_count.update(|count| *count += 1);
    });
    assert_eq!(label.borrow().text, "Count: 2");

    scope.dispose();
    set_count.set(3);
    assert_eq!(label.borrow().text, "Count: 2");
    println!("{}", label.borrow().text);
}
