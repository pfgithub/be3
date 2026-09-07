use super::*;
use crate::{Pos2, RawInput, Vec2};

#[derive(Default)]
struct CountingCounter {
    reads: Cell<usize>,
}

impl Counter for CountingCounter {
    fn value(&self) -> i64 {
        self.reads.set(self.reads.get() + 1);
        7
    }

    fn increment(&self) {}
    fn decrement(&self) {}
    fn reset(&self) {}
}

mod drawing_the_demo_does_not_poll_the_counter;
