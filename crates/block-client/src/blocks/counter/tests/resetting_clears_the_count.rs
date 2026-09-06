use super::*;

#[test]
fn resetting_clears_the_count() {
    let mut counter = Counter::new();
    Counter::apply_operation(&mut counter, &CounterOperation::Increment);
    Counter::apply_operation(&mut counter, &CounterOperation::Increment);
    Counter::apply_operation(&mut counter, &CounterOperation::Reset);
    assert_eq!(counter.count(), 0);
}
