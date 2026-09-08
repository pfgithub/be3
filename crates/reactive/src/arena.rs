use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

thread_local! {
    static ARENA: RefCell<Vec<Rc<dyn Any>>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct SlotId(u32);

pub(crate) fn insert<T: 'static>(value: T) -> SlotId {
    let boxed: Rc<dyn Any> = Rc::new(value);
    ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let id = SlotId(arena.len() as u32);
        arena.push(boxed);
        id
    })
}

pub(crate) fn get<T: 'static>(id: SlotId) -> Rc<T> {
    let boxed = ARENA.with(|arena| arena.borrow()[id.0 as usize].clone());
    boxed
        .downcast::<T>()
        .unwrap_or_else(|_| panic!("reactive arena slot type mismatch"))
}
